use std::env;
use log::info;

// Server Configuration
pub const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
pub const DATA_BROADCAST_INTERVAL_SECS: u64 = 1;
pub const CLEANUP_INTERVAL_SECS: u64 = 60;

// JWT Configuration
pub const DEFAULT_JWT_SECRET: &str = "your-secret-key-change-in-production";

// Data Configuration
pub const DEFAULT_DATA_FILE: &str = "./data/NIFTY.csv";

// Broadcast Configuration
pub const BROADCAST_CHANNEL_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct Config {
    pub jwt_secret: String,
    pub log_level: String,
    pub bind_address: String,
    pub api_bind_address: String,  // New field for API server
    pub data_file: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "your-secret-key-change-in-production".to_string()),
            log_level: env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string()),
            bind_address: env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:8080".to_string()),
            api_bind_address: env::var("API_BIND_ADDRESS")
                .unwrap_or_else(|_| "127.0.0.1:3000".to_string()),
            data_file: env::var("DATA_FILE")
                .unwrap_or_else(|_| "./data/NIFTY.csv".to_string()),
        }
    }
    
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.jwt_secret.len() < 32 {
            return Err("JWT_SECRET must be at least 32 characters long".into());
        }
        
        if !std::path::Path::new(&self.data_file).exists() {
            return Err(format!("Data file not found: {}", self.data_file).into());
        }
        
        Ok(())
    }
    
    pub fn log_config(&self) {
        info!("Configuration loaded:");
        info!("  WebSocket Server: {}", self.bind_address);
        info!("  API Server: {}", self.api_bind_address);
        info!("  Log level: {}", self.log_level);
        info!("  Data file: {}", self.data_file);
        info!("  JWT secret length: {} chars", self.jwt_secret.len());
    }
}