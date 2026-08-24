use std::env;
use log::info;

// Server Configuration
pub const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:8080";
pub const DATA_BROADCAST_INTERVAL_SECS: u64 = 1;
pub const CLEANUP_INTERVAL_SECS: u64 = 60;

// JWT Configuration
pub const DEFAULT_JWT_SECRET: &str = "3cf7753b87ed1a9e7508f9c928292bcb5fbc6441eaf587bbd8da7f17b77f4b61";

// Data Configuration
pub const DEFAULT_DATA_FILE: &str = "./data/NIFTY.csv";
pub const DEFAULT_DATA_DIR: &str = "./data";
pub const DEFAULT_TIME_SCALE_FACTOR: f64 = 0.01; // 100x faster than real time

// Broadcast Configuration
pub const BROADCAST_CHANNEL_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct Config {
    pub jwt_secret: String,
    pub log_level: String,
    pub bind_address: String,
    pub api_bind_address: String,  // New field for API server
    pub data_file: String,
    pub data_dir: String,  // Directory containing CSV files
    pub time_scale_factor: Option<f64>,  // Time scaling factor for broadcasting
    pub broadcast_interval_secs: u64,  // Interval between broadcasts
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| DEFAULT_JWT_SECRET.to_string()),
            log_level: env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string()),
            bind_address: env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_string()),
            api_bind_address: env::var("API_BIND_ADDRESS")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            data_file: env::var("DATA_FILE")
                .unwrap_or_else(|_| DEFAULT_DATA_FILE.to_string()),
            data_dir: env::var("DATA_DIR")
                .unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string()),
            time_scale_factor: env::var("TIME_SCALE_FACTOR")
                .ok()
                .and_then(|s| s.parse().ok()),
            broadcast_interval_secs: env::var("BROADCAST_INTERVAL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DATA_BROADCAST_INTERVAL_SECS),
        }
    }
    
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.jwt_secret.len() < 32 {
            return Err("JWT_SECRET must be at least 32 characters long".into());
        }
        
        // Check if data directory exists, fallback to single file check
        if !std::path::Path::new(&self.data_dir).exists() {
            if !std::path::Path::new(&self.data_file).exists() {
                return Err(format!("Neither data directory '{}' nor fallback data file '{}' exists", self.data_dir, self.data_file).into());
            }
        }
        
        // Validate time scale factor
        if let Some(factor) = self.time_scale_factor {
            if factor <= 0.0 || factor > 100.0 {
                return Err("TIME_SCALE_FACTOR must be between 0.0 and 100.0".into());
            }
        }
        
        Ok(())
    }
    
    pub fn log_config(&self) {
        info!("Configuration loaded:");
        info!("  WebSocket Server: {}", self.bind_address);
        info!("  API Server: {}", self.api_bind_address);
        info!("  Log level: {}", self.log_level);
        info!("  Data directory: {}", self.data_dir);
        info!("  Data file (fallback): {}", self.data_file);
        info!("  Time scale factor: {:?}", self.time_scale_factor);
        info!("  Broadcast interval: {} seconds", self.broadcast_interval_secs);
        info!("  JWT secret length: {} chars", self.jwt_secret.len());
    }
}