mod websocket;
mod api;
mod auth;
mod trading;
mod data;
mod config;

use std::time::Duration;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::interval;
use log::{info, error};
use tower_http::cors::CorsLayer;

use crate::config::{Config, CLEANUP_INTERVAL_SECS, BROADCAST_CHANNEL_SIZE};
use crate::data::{PubSubManager, BroadcastController};
use crate::auth::{SessionManager, extract_jwt_from_request, JwtGenerator};
use crate::websocket::{WebSocketHandler, AdminWebSocketHandler, AdminOrderEvent};
use crate::trading::OrderManager;
use crate::api::{ApiState, create_api_router};

async fn handle_websocket_connection_with_routing(
    stream: tokio::net::TcpStream,
    peer_addr: String,
    rx: broadcast::Receiver<String>,
    admin_rx: broadcast::Receiver<AdminOrderEvent>,
    session_manager: SessionManager,
    admin_session_manager: SessionManager,
    pubsub: Arc<PubSubManager>,
) {
    use tokio_tungstenite::{accept_hdr_async};
    use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
    use tokio_tungstenite::tungstenite::http::StatusCode;
    use log::{info, warn, error};

    // Capture path and handle authentication in the handshake callback
    let mut is_admin = false;
    let mut auth_claims: Option<crate::auth::Claims> = None;

    let ws_stream = match accept_hdr_async(stream, |req: &Request, response: Response| {
        let path = req.uri().path();
        info!("WebSocket connection request for path: {} from {}", path, peer_addr);
        
        match path {
            "/admin" => {
                is_admin = true;
                // Handle admin authentication
                if let Some(token) = extract_jwt_from_request(req) {
                    match admin_session_manager.validate_jwt(&token) {
                        Ok(claims) => {
                            if claims.permissions.contains(&"admin".to_string()) {
                                auth_claims = Some(claims);
                                info!("Admin WebSocket authenticated from {}", peer_addr);
                                Ok(response)
                            } else {
                                warn!("Non-admin user attempted admin WebSocket access from {}", peer_addr);
                                Err(Response::builder()
                                    .status(StatusCode::FORBIDDEN)
                                    .body(Some("Admin permissions required".to_string()))
                                    .unwrap())
                            }
                        }
                        Err(e) => {
                            warn!("Admin WebSocket authentication failed from {}: {}", peer_addr, e);
                            Err(Response::builder()
                                .status(StatusCode::UNAUTHORIZED)
                                .body(Some(e))
                                .unwrap())
                        }
                    }
                } else {
                    warn!("Admin WebSocket missing token from {}", peer_addr);
                    Err(Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(Some("Missing Authorization header or token parameter".to_string()))
                        .unwrap())
                }
            }
            "/ws" => {
                is_admin = false;
                // For normal WebSocket, we'll let the existing handler do the auth
                Ok(response)
            }
            _ => {
                warn!("Unknown WebSocket path '{}' from {}", path, peer_addr);
                Err(Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Some("Invalid WebSocket path".to_string()))
                    .unwrap())
            }
        }
    }).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed for {}: {:?}", peer_addr, e);
            return;
        }
    };

    // Route to appropriate handler
    if is_admin {
        if let Some(claims) = auth_claims {
            info!("Routing to admin WebSocket handler for {}", peer_addr);
            // Use AdminWebSocketHandler for admin connections
            let handler = AdminWebSocketHandler::new(admin_session_manager, peer_addr);
            handler.handle_admin_websocket_direct(ws_stream, admin_rx, claims).await;
        }
    } else {
        info!("Routing to normal WebSocket handler for {}", peer_addr);
        // Handle normal WebSocket with already established connection
        let handler = WebSocketHandler::new(session_manager, peer_addr);
        handler.handle_websocket_connection_direct(ws_stream, rx, pubsub).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = Config::from_env();
    
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or(&config.log_level));
    
    // Log configuration
    config.log_config();
    
    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        return Err(e.into());
    }

    // Initialize managers
    let session_manager: SessionManager = SessionManager::new(&config.jwt_secret);
    let pubsub_manager = Arc::new(PubSubManager::new(BROADCAST_CHANNEL_SIZE));
    let broadcast_controller = Arc::new(BroadcastController::new(pubsub_manager.clone()));
    
    // Initialize broadcast channel for backwards compatibility
    let (tx, _rx) = broadcast::channel(BROADCAST_CHANNEL_SIZE);
    
    // Initialize admin order events channel
    let (admin_tx, _admin_rx) = broadcast::channel::<AdminOrderEvent>(BROADCAST_CHANNEL_SIZE);
    
    // Initialize order manager with admin events
    let order_manager = Arc::new(OrderManager::new_with_admin_events(admin_tx.clone()));

    // NOTE: Broadcasting is now controlled by admin API endpoints
    // No automatic startup of broadcasting - it's controlled via /api/start-broadcast
    info!("📊 Broadcasting system ready - use /api/start-broadcast to begin data streaming");

    // Start background tasks
    start_background_tasks(
        session_manager.clone(), 
        pubsub_manager.clone(),
        order_manager.clone(),
        admin_tx.clone()
    ).await;

    // Start API server
    let api_state = ApiState {
        order_manager: order_manager.clone(),
        session_manager: session_manager.clone(),
        jwt_generator: Arc::new(JwtGenerator::new(&config.jwt_secret)),
        pubsub_manager: pubsub_manager.clone(),
        broadcast_controller,
    };
    
    let api_router = create_api_router(api_state)
        .layer(CorsLayer::permissive()); // Enable CORS for web clients

    let api_bind_address = config.api_bind_address.clone();
    let api_listener = TcpListener::bind(&api_bind_address).await?;
    info!("🌐 HTTP API server running at http://{}", api_bind_address);

    let api_server = axum::serve(api_listener, api_router);

    // Start WebSocket server
    let ws_bind_address = config.bind_address.clone();
    let ws_listener = TcpListener::bind(&ws_bind_address).await?;
    info!("🚀 WebSocket server running at ws://{} with JWT authentication", ws_bind_address);
    info!("🔗 Normal WebSocket: ws://{}/ws", ws_bind_address);
    info!("👑 Admin WebSocket: ws://{}/admin (real-time order feed)", ws_bind_address);

    // Clone session manager for different handlers
    let ws_session_manager = session_manager.clone();
    let admin_session_manager = session_manager.clone();

    // WebSocket connection loop with path-based routing
    let websocket_server = async move {
        info!("🔗 Ready to accept WebSocket connections with path routing");
        
        while let Ok((stream, addr)) = ws_listener.accept().await {
            let peer_addr = addr.to_string();
            let rx = tx.subscribe();
            let admin_rx = admin_tx.subscribe();
            let session_manager_clone = ws_session_manager.clone();
            let admin_session_manager_clone = admin_session_manager.clone();
            let pubsub_clone = pubsub_manager.clone();
            
            // Spawn connection handler with path routing
            tokio::spawn(async move {
                // We need to peek at the HTTP request to determine the path
                // This will be handled in a new router function
                handle_websocket_connection_with_routing(
                    stream,
                    peer_addr,
                    rx,
                    admin_rx,
                    session_manager_clone,
                    admin_session_manager_clone,
                    pubsub_clone
                ).await;
            });
        }
    };

    // Run both servers concurrently
    info!("🎯 Starting WebSocket and HTTP API servers...");
    tokio::select! {
        result = api_server => {
            error!("API server stopped: {:?}", result);
        }
        _ = websocket_server => {
            error!("WebSocket server stopped");
        }
    }
    
    Ok(())
}

async fn start_background_tasks(
    session_manager: SessionManager, 
    pubsub: Arc<PubSubManager>,
    order_manager: Arc<OrderManager>,
    admin_tx: broadcast::Sender<AdminOrderEvent>
) {
    // Session cleanup task
    let session_manager_cleanup = session_manager.clone();
    tokio::spawn(async move {
        let mut interval_timer = interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        
        loop {
            interval_timer.tick().await;
            session_manager_cleanup.cleanup_stale_sessions();
            
            // Log session stats
            let count = session_manager_cleanup.get_session_count();
            info!("Active sessions: {}", count);
        }
    });
    
    // Pub/sub stats task
    let pubsub_stats = pubsub.clone();
    let order_stats = order_manager.clone();
    tokio::spawn(async move {
        let mut interval_timer = interval(Duration::from_secs(60)); // Every minute
        
        loop {
            interval_timer.tick().await;
            let (symbol_count, subscription_count) = pubsub_stats.get_stats();
            let (order_count, user_count) = order_stats.get_stats();
            
            if symbol_count > 0 || subscription_count > 0 || order_count > 0 {
                info!("Stats - Symbols: {}, Subscriptions: {}, Orders: {}, Trading users: {}", 
                      symbol_count, subscription_count, order_count, user_count);
            }
        }
    });
    
    info!("🧹 Started session cleanup task (every {} seconds)", CLEANUP_INTERVAL_SECS);
    info!("📈 Started stats monitoring task (every 60 seconds)");
}
