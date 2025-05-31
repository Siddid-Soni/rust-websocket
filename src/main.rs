use std::sync::{Arc, Mutex};
use std::time::Duration;
use futures::SinkExt;
use futures::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_hdr_async};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response, ErrorResponse};
use tokio_tungstenite::tungstenite::http::StatusCode;
use log::{info, warn, error};
use tokio::sync::broadcast;
use std::fs::File;
use std::io::{self, BufRead};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// Valid authentication tokens (in a real app, this would be in a database)
const VALID_TOKENS: &[&str] = &["your-secret-token-123", "another-valid-token-456"];

// Track active tokens to ensure one connection per token
type ActiveTokens = Arc<Mutex<HashSet<String>>>;

// Function to extract token from request
fn extract_token_from_request(req: &Request) -> Option<String> {
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                if VALID_TOKENS.contains(&token) {
                    return Some(token.to_string());
                }
            }
        }
    }
    None
}

// Authentication callback function that checks query parameters or headers
fn authenticate_request(
    req: &Request, 
    response: Response, 
    active_tokens: &ActiveTokens
) -> Result<Response, ErrorResponse> {
    if let Some(token) = extract_token_from_request(req) {
        // Check if token is already in use
        let mut active = active_tokens.lock().unwrap();
        if active.contains(&token) {
            warn!("Authentication failed - token already in use: {}", token);
            let response = Response::builder()
                .status(StatusCode::CONFLICT)
                .body(Some("Token already in use".to_string()))
                .unwrap();
            return Err(response);
        }
        
        // Register the token as active
        active.insert(token.clone());
        info!("Authenticated connection with token: {}", token);
        return Ok(response);
    }
    
    warn!("Authentication failed - invalid or missing token");
    let response = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Some("Unauthorized".to_string()))
        .unwrap();
    Err(response)
}

#[derive(Debug, Deserialize, Serialize)]
struct Data {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
}

#[tokio::main]
async fn main() {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let addr = "127.0.0.1:8080";
    let listener = TcpListener::bind(addr).await.unwrap();
    info!("WebSocket server running at {}", addr);

    // Track active tokens
    let active_tokens: ActiveTokens = Arc::new(Mutex::new(HashSet::new()));

    // Broadcast channel to notify all clients of score updates
    let (tx, _rx) = broadcast::channel(10);
    
    let tx_clone = tx.clone();
    let file = File::open("./src/data.csv").unwrap();
    let lines = io::BufReader::new(file).lines();

    let mut stock_data: Vec<Data> = Vec::new();

    for line in lines {
        let line = line.unwrap();
        let line = line.split(",").collect::<Vec<&str>>();
        let data = Data {
            date: line[0].to_string(),
            open: line[1].parse().unwrap(),
            high: line[2].parse().unwrap(),
            low: line[3].parse().unwrap(),
            close: line[4].parse().unwrap(),
            volume: line[5].parse().unwrap(),
        };
        stock_data.push(data);
    }
    let length = stock_data.len();

    let stock_data = Arc::new(Mutex::new(stock_data));
    let data_clone = Arc::clone(&stock_data);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));

        for i in 0..length{
            interval.tick().await;
            let row = data_clone.lock().unwrap();
            let row = row.get(i).unwrap();
            let message = serde_json::to_string(row).unwrap();
            let _ = tx_clone.send(message.clone());
        }

        let _ = tx_clone.send(String::from("done"));
    });

    while let Ok((stream, _)) = listener.accept().await {
        let rx = tx.subscribe();
        let active_tokens_clone = Arc::clone(&active_tokens);
        tokio::spawn(handle_client(stream, rx, active_tokens_clone));
    }
}

// Custom connection handler that properly manages token lifecycle
async fn handle_client(
    stream: tokio::net::TcpStream, 
    mut rx: broadcast::Receiver<String>,
    active_tokens: ActiveTokens
) {
    // We need to handle authentication ourselves to get the token
    let peer_addr = stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap());
    
    // Handle the WebSocket handshake with authentication
    let mut extracted_token: Option<String> = None;
    
    let ws_stream = match accept_hdr_async(stream, |req: &Request, response: Response| {
        // Extract token for later cleanup
        extracted_token = extract_token_from_request(req);
        // Perform authentication
        authenticate_request(req, response, &active_tokens)
    }).await {
        Ok(ws) => ws,
        Err(e) => {
            error!("WebSocket handshake failed for {}: {:?}", peer_addr, e);
            return;
        }
    };

    // Get the token that was used for this connection
    let connection_token = match extracted_token {
        Some(token) => token,
        None => {
            error!("No token found after successful authentication - this shouldn't happen");
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Log new connection
    info!("New authenticated WebSocket connection established with token: {}", connection_token);

    // Create a channel to signal when to close the connection
    let (close_tx, mut close_rx) = tokio::sync::mpsc::channel::<()>(1);
    
    // Clone token for cleanup in tasks
    // let token_for_cleanup = connection_token.clone();
    // let active_tokens_for_cleanup = Arc::clone(&active_tokens);
    
    // Create a task to send updates to the client
    let write_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                // Listen for incoming messages from the broadcast channel
                message_result = rx.recv() => {
                    match message_result {
                        Ok(message) => {
                            if message == "done" {
                                // Send close frame with a normal closure code
                                info!("Sending close frame to client");
                                if let Err(e) = write.send(Message::Close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
                                    code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
                                    reason: "Stream completed".into(),
                                }))).await {
                                    error!("Error sending close frame: {:?}", e);
                                }
                                break;
                            } else {
                                // Send regular message
                                if let Err(e) = write.send(Message::Text(message.clone())).await {
                                    error!("Error sending message: {:?}", e);
                                    break;
                                }
                                info!("Sent message: {}", message);
                            }
                        }
                        Err(e) => {
                            error!("Error receiving from broadcast: {:?}", e);
                            break;
                        }
                    }
                }
                // Listen for close signal from read task
                _ = close_rx.recv() => {
                    info!("Received close signal from read task");
                    break;
                }
            }
        }
        info!("Write task ending");
    });

    // Handle incoming messages in the main task
    let read_task = tokio::spawn(async move {
        while let Some(msg_result) = read.next().await {
            match msg_result {
                Ok(msg) => {
                    match msg {
                        Message::Close(close_frame) => {
                            info!("Received close frame from client: {:?}", close_frame);
                            // Signal the write task to close
                            let _ = close_tx.send(()).await;
                            break;
                        }
                        Message::Ping(_data) => {
                            info!("Received ping from client");
                            // Pong responses are handled automatically by the library
                        }
                        Message::Pong(_) => {
                            info!("Received pong from client");
                        }
                        Message::Text(text) => {
                            info!("Received text message from client: {}", text);
                        }
                        Message::Binary(data) => {
                            info!("Received binary message from client: {} bytes", data.len());
                        }
                        Message::Frame(_) => {
                            // Raw frames are typically handled internally by the library
                            info!("Received raw frame from client");
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading message: {:?}", e);
                    // Signal the write task to close
                    let _ = close_tx.send(()).await;
                    break;
                }
            }
        }
        info!("Read task ending");
    });

    // Wait for both tasks to complete
    tokio::select! {
        _ = write_task => {
            info!("Write task completed first");
        }
        _ = read_task => {
            info!("Read task completed first");
        }
    }

    // Cleanup: Remove token from active tokens when connection closes
    {
        let mut active = active_tokens.lock().unwrap();
        active.remove(&connection_token);
        info!("Removed token from active set: {}", connection_token);
    }

    // Log when connection is closed
    info!("WebSocket connection closed gracefully for token: {}", connection_token);
}