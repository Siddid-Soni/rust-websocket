use std::env;
use log::warn;

// Server Configuration
pub const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
pub const DATA_BROADCAST_INTERVAL_SECS: u64 = 10;
pub const CLEANUP_INTERVAL_SECS: u64 = 60;

// JWT Configuration
pub const DEFAULT_JWT_SECRET: &str = "your-secret-key-change-in-production";

// Data Configuration
pub const DEFAULT_DATA_FILE: &str = "./src/data.csv";

// Broadcast Configuration
pub const BROADCAST_CHANNEL_SIZE: usize = 100;

pub struct Config {
    pub bind_address: String,
    pub jwt_secret: String,
    pub data_file: String,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            bind_address: env::var("BIND_ADDRESS")
                .unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_string()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| {
                    warn!("JWT_SECRET not set, using default (NOT for production!)");
                    DEFAULT_JWT_SECRET.to_string()
                }),
            data_file: env::var("DATA_FILE")
                .unwrap_or_else(|_| DEFAULT_DATA_FILE.to_string()),
            log_level: env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string()),
        }
    }
    
    pub fn validate(&self) -> Result<(), String> {
        if self.jwt_secret == DEFAULT_JWT_SECRET {
            warn!("Using default JWT secret - change for production!");
        }
        
        if self.jwt_secret.len() < 32 {
            return Err("JWT secret should be at least 32 characters long".to_string());
        }
        
        if !std::path::Path::new(&self.data_file).exists() {
            return Err(format!("Data file not found: {}", self.data_file));
        }
        
        Ok(())
    }
    
    pub fn log_config(&self) {
        println!("Server Configuration:");
        println!("  Bind Address: {}", self.bind_address);
        println!("  Data File: {}", self.data_file);
        println!("  Log Level: {}", self.log_level);
        println!("  JWT Secret: {}***", &self.jwt_secret[..4.min(self.jwt_secret.len())]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_from_env() {
        let config = Config::from_env();
        assert!(!config.bind_address.is_empty());
        assert!(!config.jwt_secret.is_empty());
        assert!(!config.data_file.is_empty());
    }
    
    #[test]
    fn test_config_validation() {
        let mut config = Config::from_env();
        config.jwt_secret = "short".to_string();
        
        assert!(config.validate().is_err());
        
        config.jwt_secret = "a".repeat(32);
        assert!(config.validate().is_ok() || !std::path::Path::new(&config.data_file).exists());
    }
} 