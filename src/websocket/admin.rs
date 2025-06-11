use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use log::{info, warn, error};
use serde_json;

use crate::auth::Claims;
use crate::auth::SessionManager;
use crate::trading::Order;

#[derive(Debug, Clone)]
pub struct AdminOrderEvent {
    pub event_type: String,
    pub order: Order,
    pub timestamp: String,
    pub user_id: String,
}

pub struct AdminWebSocketHandler {
    session_manager: SessionManager,
    peer_addr: String,
}

impl AdminWebSocketHandler {
    pub fn new(session_manager: SessionManager, peer_addr: String) -> Self {
        Self {
            session_manager,
            peer_addr,
        }
    }
    
    pub async fn handle_admin_websocket_direct(
        self,
        ws_stream: WebSocketStream<TcpStream>,
        order_events_rx: broadcast::Receiver<AdminOrderEvent>,
        claims: Claims,
    ) {
        // Verify admin permissions
        if !claims.permissions.contains(&"admin".to_string()) {
            error!("User {} does not have admin permissions", claims.user_id);
            return;
        }

        self.handle_admin_websocket_connection(ws_stream, order_events_rx, claims).await;
    }
    
    async fn handle_admin_websocket_connection(
        &self,
        ws_stream: WebSocketStream<TcpStream>,
        mut order_events_rx: broadcast::Receiver<AdminOrderEvent>,
        claims: Claims,
    ) {
        let (mut write, mut read) = ws_stream.split();

        info!("Admin WebSocket connection established - User: {}, Session: {} from {}", 
              claims.user_id, &claims.jti[..8], self.peer_addr);

        // Send welcome message
        let welcome_msg = serde_json::json!({
            "type": "admin_connected",
            "message": "Connected to admin order feed",
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
        
        if let Err(e) = write.send(Message::Text(welcome_msg.to_string())).await {
            error!("Failed to send welcome message: {}", e);
            return;
        }

        // Create channels for coordination
        let (close_tx, mut close_rx) = mpsc::channel::<()>(1);
        
        // Spawn read task to handle incoming messages (ping/pong, close, etc.)
        let read_close_tx = close_tx.clone();
        let read_task = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Handle admin commands if needed
                        info!("Admin message received: {}", text);
                    }
                    Ok(Message::Ping(_data)) => {
                        info!("Admin ping received");
                        // Pong will be handled automatically
                    }
                    Ok(Message::Close(_)) => {
                        info!("Admin close message received");
                        break;
                    }
                    Err(e) => {
                        error!("Admin WebSocket read error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            let _ = read_close_tx.send(()).await;
        });

        // Main event loop - forward order events to admin client
        loop {
            tokio::select! {
                // Receive order events and forward to admin
                order_event = order_events_rx.recv() => {
                    match order_event {
                        Ok(event) => {
                            let remaining_quantity = if event.order.filled_quantity < event.order.quantity {
                                event.order.quantity - event.order.filled_quantity
                            } else {
                                0
                            };

                            let admin_message = serde_json::json!({
                                "type": "order_event",
                                "event_type": event.event_type,
                                "order": {
                                    "id": event.order.id,
                                    "user_id": event.user_id,
                                    "symbol": event.order.symbol,
                                    "side": event.order.side,
                                    "order_type": event.order.order_type,
                                    "quantity": event.order.quantity,
                                    "price": event.order.price,
                                    "stop_price": event.order.stop_price,
                                    "status": event.order.status,
                                    "filled_quantity": event.order.filled_quantity,
                                    "remaining_quantity": remaining_quantity,
                                    "average_price": event.order.average_price,
                                    "created_at": event.order.created_at,
                                    "updated_at": event.order.updated_at
                                },
                                "timestamp": event.timestamp
                            });
                            
                            if let Err(e) = write.send(Message::Text(admin_message.to_string())).await {
                                error!("Failed to send order event to admin: {}", e);
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("Admin client lagged, skipped {} order events", skipped);
                            
                            // Send lag notification
                            let lag_msg = serde_json::json!({
                                "type": "lag_warning",
                                "message": format!("Client lagged, {} events skipped", skipped),
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            });
                            
                            if let Err(e) = write.send(Message::Text(lag_msg.to_string())).await {
                                error!("Failed to send lag warning: {}", e);
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Order events channel closed");
                            break;
                        }
                    }
                }
                
                // Handle connection close
                _ = close_rx.recv() => {
                    info!("Admin WebSocket connection closing");
                    break;
                }
            }
        }

        // Cleanup
        read_task.abort();

        info!("Admin WebSocket connection closed - User: {}, Session: {} from {}", 
              claims.user_id, &claims.jti[..8], self.peer_addr);
    }
}

// Helper functions for creating admin order events
impl AdminOrderEvent {
    pub fn new(event_type: &str, order: Order, user_id: String) -> Self {
        Self {
            event_type: event_type.to_string(),
            order,
            timestamp: chrono::Utc::now().to_rfc3339(),
            user_id,
        }
    }
    
    pub fn order_placed(order: Order, user_id: String) -> Self {
        Self::new("order_placed", order, user_id)
    }
    
    pub fn order_filled(order: Order, user_id: String) -> Self {
        Self::new("order_filled", order, user_id)
    }
    
    pub fn order_cancelled(order: Order, user_id: String) -> Self {
        Self::new("order_cancelled", order, user_id)
    }
    
    pub fn order_partial_fill(order: Order, user_id: String) -> Self {
        Self::new("order_partial_fill", order, user_id)
    }
    
    pub fn order_updated(order: Order, user_id: String) -> Self {
        Self::new("order_updated", order, user_id)
    }
} 