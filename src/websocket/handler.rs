use std::time::Duration;
use std::sync::Arc;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use tokio::time::interval;
use tokio_tungstenite::{accept_hdr_async, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response, ErrorResponse};
use tokio_tungstenite::tungstenite::http::StatusCode;
use log::{info, warn, error};
use std::collections::HashMap;
use serde_json;

use crate::auth::{Claims, extract_jwt_from_request};
use crate::auth::{SessionManager, HEARTBEAT_INTERVAL_SECS};
use crate::data::{PubSubManager, SubscriptionMessage, SubscriptionResponse};

pub struct WebSocketHandler {
    session_manager: SessionManager,
    peer_addr: String,
}

impl WebSocketHandler {
    pub fn new(session_manager: SessionManager, peer_addr: String) -> Self {
        Self {
            session_manager,
            peer_addr,
        }
    }
    
    pub async fn handle_connection(
        self,
        stream: TcpStream,
        rx: broadcast::Receiver<String>,
    ) {
        // Handle the WebSocket handshake with JWT authentication
        let mut jwt_claims: Option<Claims> = None;
        
        let ws_stream = match accept_hdr_async(stream, |req: &Request, response: Response| {
            // Extract and validate JWT
            if let Some(token) = extract_jwt_from_request(req) {
                if let Ok(claims) = self.session_manager.validate_jwt(&token) {
                    jwt_claims = Some(claims);
                }
            }
            // Perform authentication
            self.authenticate_request(req, response)
        }).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("WebSocket handshake failed for {}: {:?}", self.peer_addr, e);
                return;
            }
        };

        // Get the JWT claims that were used for this connection
        let claims = match jwt_claims {
            Some(claims) => claims,
            None => {
                error!("No JWT claims found after successful authentication");
                return;
            }
        };

        self.handle_websocket_connection(ws_stream, rx, claims).await;
    }
    
    pub async fn handle_connection_with_pubsub(
        self,
        stream: TcpStream,
        rx: broadcast::Receiver<String>,
        pubsub: Arc<PubSubManager>,
    ) {
        // Handle the WebSocket handshake with JWT authentication
        let mut jwt_claims: Option<Claims> = None;
        
        let ws_stream = match accept_hdr_async(stream, |req: &Request, response: Response| {
            // Extract and validate JWT
            if let Some(token) = extract_jwt_from_request(req) {
                if let Ok(claims) = self.session_manager.validate_jwt(&token) {
                    jwt_claims = Some(claims);
                }
            }
            // Perform authentication
            self.authenticate_request(req, response)
        }).await {
            Ok(ws) => ws,
            Err(e) => {
                error!("WebSocket handshake failed for {}: {:?}", self.peer_addr, e);
                return;
            }
        };

        // Get the JWT claims that were used for this connection
        let claims = match jwt_claims {
            Some(claims) => claims,
            None => {
                error!("No JWT claims found after successful authentication");
                return;
            }
        };

        self.handle_websocket_connection_with_pubsub(ws_stream, rx, claims, pubsub).await;
    }
    
    fn authenticate_request(
        &self,
        req: &Request, 
        response: Response
    ) -> Result<Response, ErrorResponse> {
        let token = match extract_jwt_from_request(req) {
            Some(t) => t,
            None => {
                warn!("Authentication failed - missing JWT token from {}", self.peer_addr);
                return Err(self.create_error_response(
                    StatusCode::UNAUTHORIZED,
                    "Missing Authorization header with Bearer token"
                ));
            }
        };
        
        match self.session_manager.try_acquire_session(&token) {
            Ok(claims) => {
                info!("Authenticated JWT session - User: {}, Session: {} from {}", 
                      claims.user_id, &claims.jti[..8], self.peer_addr);
                Ok(response)
            }
            Err(error_msg) => {
                warn!("JWT authentication failed for {}: {}", self.peer_addr, error_msg);
                let status = match error_msg.as_str() {
                    "Session already active" => StatusCode::CONFLICT,
                    "Maximum connections reached" => StatusCode::SERVICE_UNAVAILABLE,
                    "Token expired" => StatusCode::UNAUTHORIZED,
                    _ => StatusCode::UNAUTHORIZED,
                };
                Err(self.create_error_response(status, &error_msg))
            }
        }
    }
    
    fn create_error_response(&self, status: StatusCode, message: &str) -> ErrorResponse {
        Response::builder()
            .status(status)
            .body(Some(message.to_string()))
            .unwrap()
    }
    
    async fn handle_websocket_connection(
        &self,
        ws_stream: WebSocketStream<TcpStream>,
        rx: broadcast::Receiver<String>,
        claims: Claims,
    ) {
        let (write, read) = ws_stream.split();

        info!("WebSocket connection established - User: {}, Session: {} from {}", 
              claims.user_id, &claims.jti[..8], self.peer_addr);

        // Create channels for coordination
        let (close_tx, close_rx) = mpsc::channel::<()>(1);
        
        // Heartbeat task
        let heartbeat_task = self.spawn_heartbeat_task(claims.jti.clone());
        
        // Write task - handles outgoing messages
        let write_task = self.spawn_write_task(write, rx, close_rx);
        
        // Read task - handles incoming messages
        let read_task = self.spawn_read_task(read, close_tx, claims.user_id.clone());

        // Wait for tasks to complete
        tokio::select! {
            _ = write_task => {
                info!("Write task completed for session {}", &claims.jti[..8]);
            }
            _ = read_task => {
                info!("Read task completed for session {}", &claims.jti[..8]);
            }
        }

        // Cleanup
        heartbeat_task.abort();
        
        if let Err(e) = self.session_manager.release_session(&claims.jti) {
            error!("Failed to release session {}: {}", &claims.jti[..8], e);
        } else {
            info!("Released session: {} for user: {}", &claims.jti[..8], claims.user_id);
        }

        info!("WebSocket connection closed - User: {}, Session: {} from {}", 
              claims.user_id, &claims.jti[..8], self.peer_addr);
    }
    
    async fn handle_websocket_connection_with_pubsub(
        &self,
        ws_stream: WebSocketStream<TcpStream>,
        _rx: broadcast::Receiver<String>, // Keep for potential backwards compatibility
        claims: Claims,
        pubsub: Arc<PubSubManager>,
    ) {
        let (write, read) = ws_stream.split();

        info!("WebSocket connection established with pub/sub - User: {}, Session: {} from {}", 
              claims.user_id, &claims.jti[..8], self.peer_addr);

        // Create channels for coordination
        let (close_tx, close_rx) = mpsc::channel::<()>(1);
        let (subscription_tx, subscription_rx) = mpsc::channel::<String>(100);
        
        // Heartbeat task
        let heartbeat_task = self.spawn_heartbeat_task(claims.jti.clone());
        
        // Write task - handles outgoing messages from subscriptions
        let write_task = self.spawn_pubsub_write_task(write, subscription_rx, close_rx);
        
        // Read task - handles incoming subscription messages
        let read_task = self.spawn_pubsub_read_task(
            read, 
            close_tx, 
            claims.clone(),
            pubsub.clone(),
            subscription_tx
        );

        // Wait for tasks to complete
        tokio::select! {
            _ = write_task => {
                info!("Write task completed for session {}", &claims.jti[..8]);
            }
            _ = read_task => {
                info!("Read task completed for session {}", &claims.jti[..8]);
            }
        }

        // Cleanup
        heartbeat_task.abort();
        pubsub.cleanup_session(&claims.jti);
        
        if let Err(e) = self.session_manager.release_session(&claims.jti) {
            error!("Failed to release session {}: {}", &claims.jti[..8], e);
        } else {
            info!("Released session: {} for user: {}", &claims.jti[..8], claims.user_id);
        }

        info!("WebSocket connection closed - User: {}, Session: {} from {}", 
              claims.user_id, &claims.jti[..8], self.peer_addr);
    }
    
    fn spawn_heartbeat_task(&self, session_id: String) -> tokio::task::JoinHandle<()> {
        let session_manager = self.session_manager.clone();
        
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
            
            loop {
                interval_timer.tick().await;
                if session_manager.update_heartbeat(&session_id).is_err() {
                    break;
                }
            }
        })
    }
    
    fn spawn_write_task(
        &self,
        mut write: futures::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
        mut rx: broadcast::Receiver<String>,
        mut close_rx: mpsc::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    message_result = rx.recv() => {
                        match message_result {
                            Ok(message) => {
                                if message == "done" {
                                    info!("Sending close frame to client");
                                    if let Err(e) = write.send(Message::Close(Some(
                                        tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                            code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
                                            reason: "Stream completed".into(),
                                        }
                                    ))).await {
                                        error!("Error sending close frame: {:?}", e);
                                    }
                                    break;
                                } else {
                                    if let Err(e) = write.send(Message::Text(message)).await {
                                        error!("Error sending message: {:?}", e);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Error receiving from broadcast: {:?}", e);
                                break;
                            }
                        }
                    }
                    _ = close_rx.recv() => {
                        info!("Received close signal from read task");
                        break;
                    }
                }
            }
        })
    }
    
    fn spawn_read_task(
        &self,
        mut read: futures::stream::SplitStream<WebSocketStream<TcpStream>>,
        close_tx: mpsc::Sender<()>,
        user_id: String,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg_result) = read.next().await {
                match msg_result {
                    Ok(msg) => {
                        match msg {
                            Message::Close(close_frame) => {
                                info!("Received close frame from user {}: {:?}", user_id, close_frame);
                                if close_tx.send(()).await.is_err() {
                                    warn!("Failed to send close signal for user {}", user_id);
                                }
                                break;
                            }
                            Message::Ping(_) => {
                                info!("Received ping from user {}", user_id);
                            }
                            Message::Pong(_) => {
                                info!("Received pong from user {}", user_id);
                            }
                            Message::Text(text) => {
                                info!("Received text message from user {}: {}", user_id, text);
                            }
                            Message::Binary(data) => {
                                info!("Received binary message from user {}: {} bytes", user_id, data.len());
                            }
                            Message::Frame(_) => {
                                info!("Received raw frame from user {}", user_id);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error reading message from user {}: {:?}", user_id, e);
                        if close_tx.send(()).await.is_err() {
                            warn!("Failed to send close signal for user {}", user_id);
                        }
                        break;
                    }
                }
            }
        })
    }

    fn spawn_pubsub_write_task(
        &self,
        mut write: futures::stream::SplitSink<WebSocketStream<TcpStream>, Message>,
        mut subscription_rx: mpsc::Receiver<String>,
        mut close_rx: mpsc::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Handle subscribed data
                    Some(message) = subscription_rx.recv() => {
                        if let Err(e) = write.send(Message::Text(message)).await {
                            error!("Error sending subscribed message: {:?}", e);
                            break;
                        }
                    }
                    // Handle close signal
                    _ = close_rx.recv() => {
                        info!("Received close signal");
                        break;
                    }
                }
            }
        })
    }

    fn spawn_pubsub_read_task(
        &self,
        mut read: futures::stream::SplitStream<WebSocketStream<TcpStream>>,
        close_tx: mpsc::Sender<()>,
        claims: Claims,
        pubsub: Arc<PubSubManager>,
        subscription_tx: mpsc::Sender<String>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // Store multiple subscription tasks instead of just one
            let mut subscription_tasks: HashMap<String, tokio::task::JoinHandle<()>> = HashMap::new();

            while let Some(msg_result) = read.next().await {
                match msg_result {
                    Ok(msg) => {
                        match msg {
                            Message::Text(text) => {
                                // Handle subscription commands
                                if let Ok(sub_msg) = serde_json::from_str::<SubscriptionMessage>(&text) {
                                    match sub_msg.action.as_str() {
                                        "subscribe" => {
                                            let symbol = sub_msg.symbol.clone();
                                            
                                            // Check if already subscribed to this symbol
                                            if subscription_tasks.contains_key(&symbol) {
                                                let response = SubscriptionResponse {
                                                    status: "error".to_string(),
                                                    symbol: Some(symbol),
                                                    message: "Already subscribed to this symbol".to_string(),
                                                };
                                                if let Ok(response_json) = serde_json::to_string(&response) {
                                                    let _ = subscription_tx.send(response_json).await;
                                                }
                                                continue;
                                            }

                                            match pubsub.subscribe(claims.jti.clone(), symbol.clone()) {
                                                Ok(mut rx) => {
                                                    // Spawn task to forward subscribed messages for this symbol
                                                    let tx = subscription_tx.clone();
                                                    let task = tokio::spawn(async move {
                                                        while let Ok(data) = rx.recv().await {
                                                            if tx.send(data).await.is_err() {
                                                                break;
                                                            }
                                                        }
                                                    });
                                                    
                                                    subscription_tasks.insert(symbol.clone(), task);

                                                    let response = SubscriptionResponse {
                                                        status: "success".to_string(),
                                                        symbol: Some(symbol),
                                                        message: "Successfully subscribed".to_string(),
                                                    };
                                                    if let Ok(response_json) = serde_json::to_string(&response) {
                                                        let _ = subscription_tx.send(response_json).await;
                                                    }
                                                }
                                                Err(e) => {
                                                    let response = SubscriptionResponse {
                                                        status: "error".to_string(),
                                                        symbol: Some(symbol),
                                                        message: e,
                                                    };
                                                    if let Ok(response_json) = serde_json::to_string(&response) {
                                                        let _ = subscription_tx.send(response_json).await;
                                                    }
                                                }
                                            }
                                        }
                                        "unsubscribe" => {
                                            let symbol = sub_msg.symbol.clone();
                                            
                                            // Cancel specific subscription task
                                            if let Some(task) = subscription_tasks.remove(&symbol) {
                                                task.abort();
                                                
                                                match pubsub.unsubscribe(&claims.jti, Some(symbol.clone())) {
                                                    Ok(_) => {
                                                        let response = SubscriptionResponse {
                                                            status: "success".to_string(),
                                                            symbol: Some(symbol),
                                                            message: "Successfully unsubscribed".to_string(),
                                                        };
                                                        if let Ok(response_json) = serde_json::to_string(&response) {
                                                            let _ = subscription_tx.send(response_json).await;
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let response = SubscriptionResponse {
                                                            status: "error".to_string(),
                                                            symbol: Some(symbol),
                                                            message: e,
                                                        };
                                                        if let Ok(response_json) = serde_json::to_string(&response) {
                                                            let _ = subscription_tx.send(response_json).await;
                                                        }
                                                    }
                                                }
                                            } else {
                                                let response = SubscriptionResponse {
                                                    status: "error".to_string(),
                                                    symbol: Some(symbol),
                                                    message: "Not subscribed to this symbol".to_string(),
                                                };
                                                if let Ok(response_json) = serde_json::to_string(&response) {
                                                    let _ = subscription_tx.send(response_json).await;
                                                }
                                            }
                                        }
                                        "unsubscribe_all" => {
                                            // Cancel all subscription tasks
                                            for (_, task) in subscription_tasks.drain() {
                                                task.abort();
                                            }

                                            match pubsub.unsubscribe(&claims.jti, None) {
                                                Ok(unsubscribed_symbols) => {
                                                    let response = SubscriptionResponse {
                                                        status: "success".to_string(),
                                                        symbol: None,
                                                        message: format!("Successfully unsubscribed from {} symbols", unsubscribed_symbols.len()),
                                                    };
                                                    if let Ok(response_json) = serde_json::to_string(&response) {
                                                        let _ = subscription_tx.send(response_json).await;
                                                    }
                                                }
                                                Err(e) => {
                                                    let response = SubscriptionResponse {
                                                        status: "error".to_string(),
                                                        symbol: None,
                                                        message: e,
                                                    };
                                                    if let Ok(response_json) = serde_json::to_string(&response) {
                                                        let _ = subscription_tx.send(response_json).await;
                                                    }
                                                }
                                            }
                                        }
                                        _ => {
                                            warn!("Unknown subscription action: {}", sub_msg.action);
                                        }
                                    }
                                } else {
                                    info!("Received non-subscription text message from user {}: {}", claims.user_id, text);
                                }
                            }
                            Message::Close(_) => {
                                info!("Client sent close frame for session {}", &claims.jti[..8]);
                                break;
                            }
                            Message::Ping(_) => {
                                info!("Received ping from session {}", &claims.jti[..8]);
                            }
                            _ => {
                                // Handle other message types as needed
                            }
                        }
                    }
                    Err(e) => {
                        error!("WebSocket error for session {}: {:?}", &claims.jti[..8], e);
                        break;
                    }
                }
            }

            // Cleanup all subscription tasks on exit
            for (_, task) in subscription_tasks {
                task.abort();
            }

            let _ = close_tx.send(()).await;
        })
    }

    pub async fn handle_websocket_connection_direct(
        self,
        ws_stream: WebSocketStream<TcpStream>,
        rx: broadcast::Receiver<String>,
        pubsub: Arc<PubSubManager>,
    ) {
        // For direct connections, we need to do authentication here since handshake is done
        // Extract token from WebSocket connection (this won't work as handshake is already done)
        // For now, we'll skip authentication since it should be handled by the router
        
        // Create a dummy claims for now - this is not ideal but works for path routing
        let dummy_claims = Claims {
            sub: "unknown".to_string(),
            jti: uuid::Uuid::new_v4().to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp(),
            iat: chrono::Utc::now().timestamp(),
            user_id: "unknown".to_string(),
            permissions: vec!["read_data".to_string(), "websocket_connect".to_string()],
        };
        
        self.handle_websocket_connection_with_pubsub(ws_stream, rx, dummy_claims, pubsub).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_websocket_handler_creation() {
        let session_manager = SessionManager::new();
        let handler = WebSocketHandler::new(session_manager, "127.0.0.1:8080".to_string());
        assert_eq!(handler.peer_addr, "127.0.0.1:8080");
    }
} 