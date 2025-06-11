use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use log::{warn, error};
use urlencoding;

// JWT Configuration
const JWT_SECRET: &str = "your-secret-key-change-in-production";

// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,        // Subject (user_id)
    pub jti: String,        // JWT ID (unique session identifier)
    pub exp: i64,          // Expiration time
    pub iat: i64,          // Issued at
    pub user_id: String,   // User identifier
    pub permissions: Vec<String>, // User permissions
}

// JWT validator
pub struct JwtValidator {
    decoding_key: DecodingKey,
    validation: Validation,
}

impl JwtValidator {
    pub fn new() -> Self {
        // In production: load secret from environment variable
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            warn!("JWT_SECRET not set, using default (NOT for production!)");
            JWT_SECRET.to_string()
        });
        
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 30; // Allow 30 seconds clock skew
        
        Self {
            decoding_key: DecodingKey::from_secret(secret.as_ref()),
            validation,
        }
    }
    
    pub fn validate_token(&self, token: &str) -> Result<Claims, String> {
        match decode::<Claims>(token, &self.decoding_key, &self.validation) {
            Ok(token_data) => {
                let claims = token_data.claims;
                
                // Check if token is expired (additional check beyond library)
                let now = Utc::now().timestamp();
                if claims.exp < now {
                    return Err("Token expired".to_string());
                }
                
                // Validate required claims
                if claims.sub.is_empty() || claims.jti.is_empty() {
                    return Err("Invalid token claims".to_string());
                }
                
                Ok(claims)
            }
            Err(e) => {
                error!("JWT validation error: {:?}", e);
                Err(format!("Invalid token: {}", e))
            }
        }
    }
}

// Function to extract JWT from request
pub fn extract_jwt_from_request(req: &tokio_tungstenite::tungstenite::handshake::server::Request) -> Option<String> {
    // First try to get token from Authorization header (existing behavior)
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                let token = &auth_str[7..];
                return Some(token.to_string());
            }
        }
    }
    
    // If no header token found, try to get token from query parameter
    // This supports browser WebSocket connections which can't send custom headers
    if let Some(query) = req.uri().query() {
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "token" {
                    // URL decode the token value
                    if let Ok(decoded_token) = urlencoding::decode(value) {
                        return Some(decoded_token.to_string());
                    }
                }
            }
        }
    }
    
    None
}
