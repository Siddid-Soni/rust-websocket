use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use log::{info, warn, error};
use tokio::sync::broadcast;

use crate::websocket::AdminOrderEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderType {
    #[serde(rename = "market")]
    Market,
    #[serde(rename = "limit")]
    Limit,
    #[serde(rename = "stop_loss")]
    StopLoss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderSide {
    #[serde(rename = "buy")]
    Buy,
    #[serde(rename = "sell")]
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrderStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "filled")]
    Filled,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "rejected")]
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: u32,
    pub price: Option<f64>, // Required for limit orders, optional for market orders
    pub stop_price: Option<f64>, // Required for stop loss orders
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub user_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub quantity: u32,
    pub price: Option<f64>,
    pub stop_price: Option<f64>,
    pub status: OrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub filled_quantity: u32,
    pub average_price: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderResponse {
    pub success: bool,
    pub message: String,
    pub order: Option<Order>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderListResponse {
    pub success: bool,
    pub orders: Vec<Order>,
    pub total: usize,
}

impl OrderRequest {
    pub fn validate(&self) -> Result<(), String> {
        // Validate symbol
        if self.symbol.trim().is_empty() {
            return Err("Symbol cannot be empty".to_string());
        }

        // Validate quantity
        if self.quantity == 0 {
            return Err("Quantity must be greater than 0".to_string());
        }

        // Validate price for limit orders
        match self.order_type {
            OrderType::Limit => {
                if self.price.is_none() || self.price.unwrap() <= 0.0 {
                    return Err("Price is required and must be positive for limit orders".to_string());
                }
            }
            OrderType::StopLoss => {
                if self.stop_price.is_none() || self.stop_price.unwrap() <= 0.0 {
                    return Err("Stop price is required and must be positive for stop loss orders".to_string());
                }
            }
            OrderType::Market => {
                // Market orders don't require price validation
            }
        }

        // Validate price values if provided
        if let Some(price) = self.price {
            if price <= 0.0 {
                return Err("Price must be positive".to_string());
            }
        }

        if let Some(stop_price) = self.stop_price {
            if stop_price <= 0.0 {
                return Err("Stop price must be positive".to_string());
            }
        }

        Ok(())
    }
}

impl Order {
    pub fn new(request: OrderRequest, user_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            symbol: request.symbol.to_uppercase(),
            side: request.side,
            order_type: request.order_type,
            quantity: request.quantity,
            price: request.price,
            stop_price: request.stop_price,
            status: OrderStatus::Pending,
            created_at: now,
            updated_at: now,
            filled_quantity: 0,
            average_price: None,
        }
    }

    pub fn cancel(&mut self) {
        self.status = OrderStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    pub fn fill(&mut self, fill_price: f64, fill_quantity: u32) {
        self.filled_quantity += fill_quantity;
        self.average_price = Some(fill_price);
        
        if self.filled_quantity >= self.quantity {
            self.status = OrderStatus::Filled;
        }
        
        self.updated_at = Utc::now();
    }
}

pub struct OrderManager {
    orders: Arc<Mutex<HashMap<Uuid, Order>>>,
    user_orders: Arc<Mutex<HashMap<String, Vec<Uuid>>>>,
    admin_event_tx: Option<broadcast::Sender<AdminOrderEvent>>,
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(Mutex::new(HashMap::new())),
            user_orders: Arc::new(Mutex::new(HashMap::new())),
            admin_event_tx: None,
        }
    }

    pub fn new_with_admin_events(admin_tx: broadcast::Sender<AdminOrderEvent>) -> Self {
        Self {
            orders: Arc::new(Mutex::new(HashMap::new())),
            user_orders: Arc::new(Mutex::new(HashMap::new())),
            admin_event_tx: Some(admin_tx),
        }
    }

    fn broadcast_admin_event(&self, event: AdminOrderEvent) {
        if let Some(ref tx) = self.admin_event_tx {
            if let Err(e) = tx.send(event) {
                warn!("Failed to broadcast admin order event: {}", e);
            }
        }
    }

    pub fn place_order(&self, request: OrderRequest, user_id: String) -> Result<Order, String> {
        // Validate the order request
        request.validate()?;

        let order = Order::new(request, user_id.clone());
        let order_id = order.id;

        let mut orders = self.orders.lock()
            .map_err(|_| "Failed to acquire orders lock".to_string())?;
        
        let mut user_orders = self.user_orders.lock()
            .map_err(|_| "Failed to acquire user orders lock".to_string())?;

        // Store the order
        orders.insert(order_id, order.clone());

        // Add to user's order list
        user_orders.entry(user_id.clone())
            .or_insert_with(Vec::new)
            .push(order_id);

        info!("Order placed - ID: {}, User: {}, Symbol: {}, Side: {:?}, Quantity: {}", 
              order_id, user_id, order.symbol, order.side, order.quantity);

        // Broadcast admin event
        self.broadcast_admin_event(
            AdminOrderEvent::order_placed(order.clone(), user_id.clone())
        );

        Ok(order)
    }

    pub fn get_order(&self, order_id: Uuid) -> Option<Order> {
        self.orders.lock()
            .ok()?
            .get(&order_id)
            .cloned()
    }

    pub fn get_user_orders(&self, user_id: &str) -> Vec<Order> {
        let orders_guard = match self.orders.lock() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        let user_orders_guard = match self.user_orders.lock() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        if let Some(order_ids) = user_orders_guard.get(user_id) {
            order_ids.iter()
                .filter_map(|id| orders_guard.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn cancel_order(&self, order_id: Uuid, user_id: &str) -> Result<Order, String> {
        let mut orders = self.orders.lock()
            .map_err(|_| "Failed to acquire orders lock".to_string())?;

        let order = orders.get_mut(&order_id)
            .ok_or("Order not found".to_string())?;

        // Check if user owns this order
        if order.user_id != user_id {
            return Err("Unauthorized: You can only cancel your own orders".to_string());
        }

        // Check if order can be cancelled
        match order.status {
            OrderStatus::Pending => {
                order.cancel();
                info!("Order cancelled - ID: {}, User: {}", order_id, user_id);
                
                // Broadcast admin event
                self.broadcast_admin_event(
                    AdminOrderEvent::order_cancelled(order.clone(), user_id.to_string())
                );
                
                Ok(order.clone())
            }
            _ => Err("Order cannot be cancelled in current status".to_string())
        }
    }

    pub fn get_orders_by_symbol(&self, symbol: &str) -> Vec<Order> {
        self.orders.lock()
            .map(|orders| {
                orders.values()
                    .filter(|order| order.symbol.eq_ignore_ascii_case(symbol))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_stats(&self) -> (usize, usize) {
        let total_orders = self.orders.lock()
            .map(|orders| orders.len())
            .unwrap_or(0);

        let total_users = self.user_orders.lock()
            .map(|user_orders| user_orders.len())
            .unwrap_or(0);

        (total_orders, total_users)
    }
} 