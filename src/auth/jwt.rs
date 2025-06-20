use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation, Algorithm};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use log::error;
use urlencoding;
use uuid::Uuid;

// JWT Configuration
const TOKEN_EXPIRY_HOURS: i64 = 72; // 24 hours

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
    pub fn new(jwt_secret: &str) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.leeway = 30; // Allow 30 seconds clock skew
        
        Self {
            decoding_key: DecodingKey::from_secret(jwt_secret.as_ref()),
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

// JWT generator
pub struct JwtGenerator {
    encoding_key: EncodingKey,
}

impl JwtGenerator {
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(jwt_secret.as_ref()),
        }
    }
    
    pub fn generate_token(&self, user_id: &str, permissions: Vec<String>) -> Result<String, String> {
        let now = Utc::now();
        let exp = now.timestamp() + (TOKEN_EXPIRY_HOURS * 3600);
        
        let claims = Claims {
            sub: user_id.to_string(),
            jti: Uuid::new_v4().to_string(), // Unique session ID
            exp,
            iat: now.timestamp(),
            user_id: user_id.to_string(),
            permissions,
        };
        
        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| format!("Failed to generate token: {}", e))
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
