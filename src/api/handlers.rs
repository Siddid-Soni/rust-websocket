use std::sync::Arc;
use axum::{
    extract::{Path, State, Query},
    http::{StatusCode, HeaderMap},
    response::Json,
    routing::{get, post, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use log::{info, warn, error};

use crate::trading::{OrderManager, OrderRequest, OrderResponse, OrderListResponse, OrderStatus};
use crate::auth::{SessionManager, JwtGenerator};
use crate::auth::Claims;
use crate::data::{PubSubManager, BroadcastController, BroadcastCommand, BroadcastState};


#[derive(Clone)]
pub struct ApiState {
    pub order_manager: Arc<OrderManager>,
    pub session_manager: SessionManager,
    pub jwt_generator: Arc<JwtGenerator>,
    pub pubsub_manager: Arc<PubSubManager>,
    pub broadcast_controller: Arc<BroadcastController>,
}

#[derive(Debug, Deserialize)]
pub struct OrderQuery {
    pub symbol: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

// Request/Response structures for authentication
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub token: Option<String>,
    pub user_id: Option<String>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct BroadcastResponse {
    pub success: bool,
    pub message: String,
}

// Extract JWT token from Authorization header
fn extract_jwt_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|auth_header| auth_header.to_str().ok())
        .and_then(|auth_str| {
            if auth_str.starts_with("Bearer ") {
                Some(auth_str.trim_start_matches("Bearer ").to_string())
            } else {
                None
            }
        })
}

// Authenticate request and extract user claims
fn authenticate_request(headers: &HeaderMap, session_manager: &SessionManager) -> Result<Claims, (StatusCode, &'static str)> {
    let token = extract_jwt_from_headers(headers)
        .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

    session_manager.validate_jwt(&token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token"))
}

// POST /api/orders - Place a new order
pub async fn place_order(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(order_request): Json<OrderRequest>,
) -> Result<Json<OrderResponse>, (StatusCode, Json<OrderResponse>)> {
    // Authenticate user
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(OrderResponse {
                success: false,
                message: msg.to_string(),
                order: None,
            }))
        })?;

    // Place the order
    match state.order_manager.place_order(order_request, claims.user_id) {
        Ok(order) => {
            info!("Order placed successfully: {}", order.id);
            Ok(Json(OrderResponse {
                success: true,
                message: "Order placed successfully".to_string(),
                order: Some(order),
            }))
        }
        Err(e) => {
            warn!("Failed to place order: {}", e);
            Err((StatusCode::BAD_REQUEST, Json(OrderResponse {
                success: false,
                message: e,
                order: None,
            })))
        }
    }
}

// GET /api/orders - Get user's orders
pub async fn get_orders(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<OrderQuery>,
) -> Result<Json<OrderListResponse>, (StatusCode, Json<OrderListResponse>)> {
    // Authenticate user
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(OrderListResponse {
                success: false,
                orders: Vec::new(),
                total: 0,
            }))
        })?;

    // Get user's orders
    let mut orders = state.order_manager.get_user_orders(&claims.user_id);

    // Filter by symbol if provided
    if let Some(symbol) = query.symbol {
        orders.retain(|order| order.symbol.eq_ignore_ascii_case(&symbol));
    }

    // Filter by status if provided
    if let Some(status) = query.status {
        orders.retain(|order| {
            match status.to_lowercase().as_str() {
                "pending" => matches!(order.status, OrderStatus::Pending),
                "filled" => matches!(order.status, OrderStatus::Filled),
                "cancelled" => matches!(order.status, OrderStatus::Cancelled),
                "rejected" => matches!(order.status, OrderStatus::Rejected),
                _ => true,
            }
        });
    }

    // Sort by created_at (newest first)
    orders.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply limit if provided
    if let Some(limit) = query.limit {
        orders.truncate(limit);
    }

    let total = orders.len();

    Ok(Json(OrderListResponse {
        success: true,
        orders,
        total,
    }))
}

// GET /api/orders/{order_id} - Get specific order
pub async fn get_order(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
) -> Result<Json<OrderResponse>, (StatusCode, Json<OrderResponse>)> {
    // Authenticate user
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(OrderResponse {
                success: false,
                message: msg.to_string(),
                order: None,
            }))
        })?;

    // Parse order ID
    let order_uuid = Uuid::parse_str(&order_id)
        .map_err(|_| {
            (StatusCode::BAD_REQUEST, Json(OrderResponse {
                success: false,
                message: "Invalid order ID format".to_string(),
                order: None,
            }))
        })?;

    // Get the order
    match state.order_manager.get_order(order_uuid) {
        Some(order) => {
            // Check if user owns this order
            if order.user_id != claims.user_id {
                return Err((StatusCode::FORBIDDEN, Json(OrderResponse {
                    success: false,
                    message: "You can only view your own orders".to_string(),
                    order: None,
                })));
            }

            Ok(Json(OrderResponse {
                success: true,
                message: "Order retrieved successfully".to_string(),
                order: Some(order),
            }))
        }
        None => {
            Err((StatusCode::NOT_FOUND, Json(OrderResponse {
                success: false,
                message: "Order not found".to_string(),
                order: None,
            })))
        }
    }
}

// DELETE /api/orders/{order_id} - Cancel an order
pub async fn cancel_order(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
) -> Result<Json<OrderResponse>, (StatusCode, Json<OrderResponse>)> {
    // Authenticate user
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(OrderResponse {
                success: false,
                message: msg.to_string(),
                order: None,
            }))
        })?;

    // Parse order ID
    let order_uuid = Uuid::parse_str(&order_id)
        .map_err(|_| {
            (StatusCode::BAD_REQUEST, Json(OrderResponse {
                success: false,
                message: "Invalid order ID format".to_string(),
                order: None,
            }))
        })?;

    // Cancel the order
    match state.order_manager.cancel_order(order_uuid, &claims.user_id) {
        Ok(order) => {
            info!("Order cancelled successfully: {}", order.id);
            Ok(Json(OrderResponse {
                success: true,
                message: "Order cancelled successfully".to_string(),
                order: Some(order),
            }))
        }
        Err(e) => {
            warn!("Failed to cancel order {}: {}", order_id, e);
            let status_code = if e.contains("not found") {
                StatusCode::NOT_FOUND
            } else if e.contains("Unauthorized") {
                StatusCode::FORBIDDEN
            } else {
                StatusCode::BAD_REQUEST
            };

            Err((status_code, Json(OrderResponse {
                success: false,
                message: e,
                order: None,
            })))
        }
    }
}

// POST /api/login - Get JWT token for username
pub async fn login(
    State(state): State<ApiState>,
    Json(login_request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, Json<LoginResponse>)> {
    let username = login_request.username.trim();
    
    // Basic validation
    if username.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(LoginResponse {
            success: false,
            message: "Username cannot be empty".to_string(),
            token: None,
            user_id: None,
            permissions: None,
        })));
    }
    
    // Determine user permissions based on username
    // In a real app, this would query a user database
    let permissions = vec!["user".to_string()];
    
    // Generate JWT token
    match state.jwt_generator.generate_token(username, permissions.clone()) {
        Ok(token) => {
            info!("JWT token generated for user: {}", username);
            Ok(Json(LoginResponse {
                success: true,
                message: "Token generated successfully".to_string(),
                token: Some(token),
                user_id: Some(username.to_string()),
                permissions: Some(permissions),
            }))
        }
        Err(e) => {
            error!("Failed to generate token for user {}: {}", username, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(LoginResponse {
                success: false,
                message: "Failed to generate token".to_string(),
                token: None,
                user_id: None,
                permissions: None,
            })))
        }
    }
}

// POST /api/start-broadcast - Start data broadcasting (admin only)
pub async fn start_broadcast(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<BroadcastResponse>, (StatusCode, Json<BroadcastResponse>)> {
    // Authenticate user and check admin permissions
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(BroadcastResponse {
                success: false,
                message: msg.to_string(),
            }))
        })?;

    // Check if user has admin permissions
    if !claims.permissions.contains(&"admin".to_string()) {
        return Err((StatusCode::FORBIDDEN, Json(BroadcastResponse {
            success: false,
            message: "Admin permissions required".to_string(),
        })));
    }

    // Log the broadcast start request
    info!("Admin user {} requested to start data broadcasting", claims.user_id);
    
    // Use the broadcast controller to start broadcasting
    match state.broadcast_controller.execute_command(BroadcastCommand::Start) {
        Ok(message) => {
            Ok(Json(BroadcastResponse {
                success: true,
                message,
            }))
        }
        Err(error_message) => {
            error!("Failed to start broadcasting: {}", error_message);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(BroadcastResponse {
                success: false,
                message: error_message,
            })))
        }
    }
}

// POST /api/pause-broadcast - Pause data broadcasting (admin only)
pub async fn pause_broadcast(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<BroadcastResponse>, (StatusCode, Json<BroadcastResponse>)> {
    // Authenticate user and check admin permissions
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(BroadcastResponse {
                success: false,
                message: msg.to_string(),
            }))
        })?;

    if !claims.permissions.contains(&"admin".to_string()) {
        return Err((StatusCode::FORBIDDEN, Json(BroadcastResponse {
            success: false,
            message: "Admin permissions required".to_string(),
        })));
    }

    info!("Admin user {} requested to pause data broadcasting", claims.user_id);
    
    match state.broadcast_controller.execute_command(BroadcastCommand::Pause) {
        Ok(message) => {
            Ok(Json(BroadcastResponse {
                success: true,
                message,
            }))
        }
        Err(error_message) => {
            Err((StatusCode::BAD_REQUEST, Json(BroadcastResponse {
                success: false,
                message: error_message,
            })))
        }
    }
}

// POST /api/resume-broadcast - Resume data broadcasting (admin only)
pub async fn resume_broadcast(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<BroadcastResponse>, (StatusCode, Json<BroadcastResponse>)> {
    // Authenticate user and check admin permissions
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(BroadcastResponse {
                success: false,
                message: msg.to_string(),
            }))
        })?;

    if !claims.permissions.contains(&"admin".to_string()) {
        return Err((StatusCode::FORBIDDEN, Json(BroadcastResponse {
            success: false,
            message: "Admin permissions required".to_string(),
        })));
    }

    info!("Admin user {} requested to resume data broadcasting", claims.user_id);
    
    match state.broadcast_controller.execute_command(BroadcastCommand::Resume) {
        Ok(message) => {
            Ok(Json(BroadcastResponse {
                success: true,
                message,
            }))
        }
        Err(error_message) => {
            Err((StatusCode::BAD_REQUEST, Json(BroadcastResponse {
                success: false,
                message: error_message,
            })))
        }
    }
}

// POST /api/stop-broadcast - Stop data broadcasting (admin only)
pub async fn stop_broadcast(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<BroadcastResponse>, (StatusCode, Json<BroadcastResponse>)> {
    // Authenticate user and check admin permissions
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(BroadcastResponse {
                success: false,
                message: msg.to_string(),
            }))
        })?;

    if !claims.permissions.contains(&"admin".to_string()) {
        return Err((StatusCode::FORBIDDEN, Json(BroadcastResponse {
            success: false,
            message: "Admin permissions required".to_string(),
        })));
    }

    info!("Admin user {} requested to stop data broadcasting", claims.user_id);
    
    match state.broadcast_controller.execute_command(BroadcastCommand::Stop) {
        Ok(message) => {
            Ok(Json(BroadcastResponse {
                success: true,
                message,
            }))
        }
        Err(error_message) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(BroadcastResponse {
                success: false,
                message: error_message,
            })))
        }
    }
}

// POST /api/restart-broadcast - Restart data broadcasting (admin only)
pub async fn restart_broadcast(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<BroadcastResponse>, (StatusCode, Json<BroadcastResponse>)> {
    // Authenticate user and check admin permissions
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(BroadcastResponse {
                success: false,
                message: msg.to_string(),
            }))
        })?;

    if !claims.permissions.contains(&"admin".to_string()) {
        return Err((StatusCode::FORBIDDEN, Json(BroadcastResponse {
            success: false,
            message: "Admin permissions required".to_string(),
        })));
    }

    info!("Admin user {} requested to restart data broadcasting", claims.user_id);
    
    match state.broadcast_controller.execute_command(BroadcastCommand::Restart) {
        Ok(message) => {
            Ok(Json(BroadcastResponse {
                success: true,
                message,
            }))
        }
        Err(error_message) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(BroadcastResponse {
                success: false,
                message: error_message,
            })))
        }
    }
}

// GET /api/broadcast-status - Get broadcasting status (admin only)
#[derive(Debug, Serialize)]
pub struct BroadcastStatusResponse {
    pub success: bool,
    pub state: BroadcastState,
    pub symbol_count: usize,
    pub total_records: usize,
    pub message: String,
}

pub async fn broadcast_status(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<BroadcastStatusResponse>, (StatusCode, Json<BroadcastStatusResponse>)> {
    // Authenticate user and check admin permissions
    let claims = authenticate_request(&headers, &state.session_manager)
        .map_err(|(status, msg)| {
            (status, Json(BroadcastStatusResponse {
                success: false,
                state: BroadcastState::Stopped,
                symbol_count: 0,
                total_records: 0,
                message: msg.to_string(),
            }))
        })?;

    if !claims.permissions.contains(&"admin".to_string()) {
        return Err((StatusCode::FORBIDDEN, Json(BroadcastStatusResponse {
            success: false,
            state: BroadcastState::Stopped,
            symbol_count: 0,
            total_records: 0,
            message: "Admin permissions required".to_string(),
        })));
    }

    let (state_info, symbol_count, total_records) = state.broadcast_controller.get_status_info();
    
    Ok(Json(BroadcastStatusResponse {
        success: true,
        state: state_info.clone(),
        symbol_count,
        total_records,
        message: format!("Broadcasting is {:?} with {} symbols and {} total records", state_info, symbol_count, total_records),
    }))
}

// GET /api/health - Health check endpoint
pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "nse_socket_api",
        "timestamp": chrono::Utc::now()
    }))
}

// Create the API router
pub fn create_api_router(state: ApiState) -> Router {
    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/login", post(login))
        .route("/start-broadcast", post(start_broadcast))
        .route("/pause-broadcast", post(pause_broadcast))
        .route("/resume-broadcast", post(resume_broadcast))
        .route("/stop-broadcast", post(stop_broadcast))
        .route("/restart-broadcast", post(restart_broadcast))
        .route("/broadcast-status", get(broadcast_status))
        .route("/orders", post(place_order))
        .route("/orders", get(get_orders))
        .route("/orders/:order_id", get(get_order))
        .route("/orders/:order_id", delete(cancel_order))
        .with_state(state);

    Router::new()
        .nest("/api", api_routes)
} 