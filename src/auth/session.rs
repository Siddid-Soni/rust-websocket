use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;
use log::{warn, info, error};
use uuid::Uuid;

use crate::auth::jwt::{Claims, JwtValidator};

// Configuration constants
pub const MAX_CONNECTIONS: usize = 1000;
pub const CONNECTION_TIMEOUT_SECS: u64 = 300; // 5 minutes
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

// Connection tracking with JWT metadata
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub session_id: String,     // JWT ID (jti)
    pub user_id: String,        // User identifier
    pub connected_at: Instant,
    pub last_heartbeat: Instant,
    pub permissions: Vec<String>,
}

impl ConnectionInfo {
    pub fn new(claims: &Claims) -> Self {
        let now = Instant::now();
        Self {
            session_id: claims.jti.clone(),
            user_id: claims.user_id.clone(),
            connected_at: now,
            last_heartbeat: now,
            permissions: claims.permissions.clone(),
        }
    }
    
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }
    
    pub fn is_stale(&self, timeout: Duration) -> bool {
        Instant::now().duration_since(self.last_heartbeat) > timeout
    }
}

// Production-ready session manager using JWT
#[derive(Clone)]
pub struct SessionManager {
    active_sessions: Arc<Mutex<HashMap<String, ConnectionInfo>>>,
    jwt_validator: Arc<JwtValidator>,
}

impl SessionManager {
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            jwt_validator: Arc::new(JwtValidator::new(jwt_secret)),
        }
    }
    
    pub fn try_acquire_session(&self, token: &str) -> Result<Claims, String> {
        // Validate JWT first
        let claims = self.jwt_validator.validate_token(token)?;
        
        let mut sessions = self.active_sessions.lock()
            .map_err(|_| "Session lock poisoned".to_string())?;
            
        // Check current connection count
        if sessions.len() >= MAX_CONNECTIONS {
            return Err("Maximum connections reached".to_string());
        }
        
        // Check if session (jti) is already active
        if sessions.contains_key(&claims.jti) {
            return Err("Session already active".to_string());
        }
        
        // Register the session
        let connection_info = ConnectionInfo::new(&claims);
        sessions.insert(claims.jti.clone(), connection_info);
        
        Ok(claims)
    }
    
    pub fn release_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.active_sessions.lock()
            .map_err(|_| "Session lock poisoned".to_string())?;
        sessions.remove(session_id);
        Ok(())
    }
    
    pub fn update_heartbeat(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.active_sessions.lock()
            .map_err(|_| "Session lock poisoned".to_string())?;
        if let Some(session_info) = sessions.get_mut(session_id) {
            session_info.update_heartbeat();
        }
        Ok(())
    }
    
    pub fn cleanup_stale_sessions(&self) -> usize {
        let mut sessions = match self.active_sessions.lock() {
            Ok(sess) => sess,
            Err(_) => return 0,
        };
        
        let timeout = Duration::from_secs(CONNECTION_TIMEOUT_SECS);
        let initial_count = sessions.len();
        
        sessions.retain(|_, session_info: &mut ConnectionInfo| !session_info.is_stale(timeout));
        
        let cleaned_count = initial_count - sessions.len();
        if cleaned_count > 0 {
            warn!("Cleaned up {} stale sessions", cleaned_count);
        }
        cleaned_count
    }
    
    pub fn get_session_count(&self) -> usize {
        self.active_sessions.lock()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
    }
    
    pub fn get_user_sessions(&self, user_id: &str) -> Vec<String> {
        self.active_sessions.lock()
            .map(|sessions| {
                sessions.values()
                    .filter(|info| info.user_id == user_id)
                    .map(|info| info.session_id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
    
    pub fn get_session_info(&self, session_id: &str) -> Option<ConnectionInfo> {
        self.active_sessions.lock()
            .ok()?
            .get(session_id)
            .cloned()
    }
    
    pub fn validate_jwt(&self, token: &str) -> Result<Claims, String> {
        self.jwt_validator.validate_token(token)
    }
    
    pub fn log_session_stats(&self) {
        let count = self.get_session_count();
        info!("Active sessions: {}/{}", count, MAX_CONNECTIONS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_manager_creation() {
        let manager = SessionManager::new("test-secret");
        assert_eq!(manager.get_session_count(), 0);
    }
    
    #[test]
    fn test_connection_info_heartbeat() {
        use crate::auth::jwt::Claims;
        
        let claims = Claims {
            sub: "test".to_string(),
            jti: "test-session".to_string(),
            exp: 0,
            iat: 0,
            user_id: "test".to_string(),
            permissions: vec![],
        };
        
        let mut conn_info = ConnectionInfo::new(&claims);
        let initial_heartbeat = conn_info.last_heartbeat;
        
        // Small delay to ensure time difference
        std::thread::sleep(std::time::Duration::from_millis(1));
        conn_info.update_heartbeat();
        
        assert!(conn_info.last_heartbeat > initial_heartbeat);
    }
} 