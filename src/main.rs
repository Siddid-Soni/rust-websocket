mod jwt;
mod session;
mod data;
mod websocket;
mod config;

use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::interval;
use log::{info, error};

use crate::config::{Config, DATA_BROADCAST_INTERVAL_SECS, CLEANUP_INTERVAL_SECS, BROADCAST_CHANNEL_SIZE};
use crate::data::{DataLoader, DataBroadcaster};
use crate::session::SessionManager;
use crate::websocket::WebSocketHandler;

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

    // Bind TCP listener
    let listener = TcpListener::bind(&config.bind_address).await?;
    info!("🚀 WebSocket server running at {} with JWT authentication", config.bind_address);

    // Initialize session manager
    let session_manager = SessionManager::new();

    // Initialize broadcast channel
    let (tx, _rx) = broadcast::channel(BROADCAST_CHANNEL_SIZE);
    
    // Load and start broadcasting stock data
    let stock_data = DataLoader::load_from_csv(&config.data_file)?;
    let data_count = stock_data.len();
    
    let data_broadcaster = DataBroadcaster::new(stock_data, DATA_BROADCAST_INTERVAL_SECS);
    data_broadcaster.start_broadcasting(tx.clone());
    
    info!("📊 Started broadcasting {} stock records every {} seconds", 
          data_count, DATA_BROADCAST_INTERVAL_SECS);

    // Start background tasks
    start_background_tasks(session_manager.clone()).await;

    // Main connection loop
    info!("🔗 Ready to accept WebSocket connections");
    
    while let Ok((stream, addr)) = listener.accept().await {
        let peer_addr = addr.to_string();
        let rx = tx.subscribe();
        let session_manager_clone = session_manager.clone();
        
        // Spawn connection handler
        tokio::spawn(async move {
            let handler = WebSocketHandler::new(session_manager_clone, peer_addr);
            handler.handle_connection(stream, rx).await;
        });
    }
    
    Ok(())
}

async fn start_background_tasks(session_manager: SessionManager) {
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
    
    info!("🧹 Started session cleanup task (every {} seconds)", CLEANUP_INTERVAL_SECS);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_server_components() {
        // Test that all components can be initialized
        let config = Config::from_env();
        let session_manager = SessionManager::new();
        let (tx, _rx) = broadcast::channel(10);
        
        // These should not panic
        assert!(tx.send("test".to_string()).is_ok());
        assert_eq!(session_manager.get_session_count(), 0);
    }
}