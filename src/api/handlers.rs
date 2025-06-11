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
use crate::auth::SessionManager;
use crate::auth::Claims;

#[derive(Clone)]
pub struct ApiState {
    pub order_manager: Arc<OrderManager>,
    pub session_manager: SessionManager,
}

#[derive(Debug, Deserialize)]
pub struct OrderQuery {
    pub symbol: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
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
        .route("/orders", post(place_order))
        .route("/orders", get(get_orders))
        .route("/orders/:order_id", get(get_order))
        .route("/orders/:order_id", delete(cancel_order))
        .with_state(state);

    Router::new()
        .nest("/api", api_routes)
} 