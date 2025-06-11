use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};

#[derive(Debug, Deserialize, Serialize)]
pub struct SubscriptionMessage {
    pub action: String, // "subscribe" | "unsubscribe"
    pub symbol: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SubscriptionResponse {
    pub status: String,
    pub symbol: Option<String>,
    pub message: String,
}

pub struct PubSubManager {
    // Symbol -> Broadcast channel for that symbol
    symbol_channels: Arc<Mutex<HashMap<String, broadcast::Sender<String>>>>,
    // Session ID -> Set of symbols they're subscribed to
    session_subscriptions: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    channel_capacity: usize,
}

impl PubSubManager {
    pub fn new(channel_capacity: usize) -> Self {
        Self {
            symbol_channels: Arc::new(Mutex::new(HashMap::new())),
            session_subscriptions: Arc::new(Mutex::new(HashMap::new())),
            channel_capacity,
        }
    }

    pub fn subscribe(&self, session_id: String, symbol: String) -> Result<broadcast::Receiver<String>, String> {
        let mut channels = self.symbol_channels.lock()
            .map_err(|_| "Lock poisoned".to_string())?;
        let mut sessions = self.session_subscriptions.lock()
            .map_err(|_| "Lock poisoned".to_string())?;

        // Check if already subscribed to this symbol
        if let Some(current_symbols) = sessions.get(&session_id) {
            if current_symbols.contains(&symbol) {
                return Err(format!("Session {} already subscribed to {}", session_id, symbol));
            }
        }

        // Create channel for symbol if it doesn't exist
        if !channels.contains_key(&symbol) {
            let (tx, _) = broadcast::channel(self.channel_capacity);
            channels.insert(symbol.clone(), tx);
            info!("Created new broadcast channel for symbol: {}", symbol);
        }

        // Get receiver for the symbol
        let rx = channels.get(&symbol)
            .ok_or("Failed to get channel for symbol".to_string())?
            .subscribe();

        // Update session subscription
        sessions.entry(session_id.clone())
            .or_insert_with(HashSet::new)
            .insert(symbol.clone());

        info!("Session {} subscribed to symbol: {}", session_id, symbol);
        Ok(rx)
    }

    pub fn unsubscribe(&self, session_id: &str, symbol: Option<String>) -> Result<Vec<String>, String> {
        let mut sessions = self.session_subscriptions.lock()
            .map_err(|_| "Lock poisoned".to_string())?;

        if let Some(symbol) = symbol {
            // Unsubscribe from specific symbol
            if let Some(current_symbols) = sessions.get_mut(session_id) {
                if current_symbols.remove(&symbol) {
                    info!("Session {} unsubscribed from symbol: {}", session_id, symbol);
                    // Remove session entry if no more subscriptions
                    if current_symbols.is_empty() {
                        sessions.remove(session_id);
                    }
                    Ok(vec![symbol])
                } else {
                    Err(format!("Session {} not subscribed to {}", session_id, symbol))
                }
            } else {
                Err(format!("Session {} has no subscriptions", session_id))
            }
        } else {
            // Unsubscribe from all symbols
            if let Some(symbols) = sessions.remove(session_id) {
                let symbol_list: Vec<String> = symbols.into_iter().collect();
                info!("Session {} unsubscribed from all symbols: {:?}", session_id, symbol_list);
                Ok(symbol_list)
            } else {
                Ok(vec![]) // Not subscribed to anything
            }
        }
    }

    pub fn broadcast_to_symbol(&self, symbol: &str, data: &str) -> Result<usize, String> {
        let channels = self.symbol_channels.lock()
            .map_err(|_| "Lock poisoned".to_string())?;

        if let Some(tx) = channels.get(symbol) {
            match tx.send(data.to_string()) {
                Ok(subscriber_count) => {
                    if subscriber_count > 0 {
                        info!("Broadcasted {} data to {} subscribers", symbol, subscriber_count);
                    }
                    Ok(subscriber_count)
                }
                Err(_) => {
                    warn!("No active receivers for symbol: {}", symbol);
                    Ok(0)
                }
            }
        } else {
            // No channel exists for this symbol yet
            Ok(0)
        }
    }

    pub fn get_subscriber_count(&self, symbol: &str) -> usize {
        if let Ok(channels) = self.symbol_channels.lock() {
            if let Some(tx) = channels.get(symbol) {
                return tx.receiver_count();
            }
        }
        0
    }

    pub fn get_current_subscriptions(&self, session_id: &str) -> HashSet<String> {
        self.session_subscriptions.lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).cloned())
            .unwrap_or_default()
    }

    // Keep this method for backward compatibility
    pub fn get_current_subscription(&self, session_id: &str) -> Option<String> {
        self.session_subscriptions.lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).and_then(|symbols| symbols.iter().next().cloned()))
    }

    pub fn is_subscribed(&self, session_id: &str, symbol: &str) -> bool {
        self.session_subscriptions.lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).map(|symbols| symbols.contains(symbol)))
            .unwrap_or(false)
    }

    pub fn get_subscription_count(&self, session_id: &str) -> usize {
        self.session_subscriptions.lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).map(|symbols| symbols.len()))
            .unwrap_or(0)
    }

    pub fn cleanup_session(&self, session_id: &str) {
        let _ = self.unsubscribe(session_id, None);
    }

    pub fn get_symbol_list(&self) -> Vec<String> {
        self.symbol_channels.lock()
            .map(|channels| channels.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_stats(&self) -> (usize, usize) {
        let symbol_count = self.symbol_channels.lock()
            .map(|channels| channels.len())
            .unwrap_or(0);
        
        let session_count = self.session_subscriptions.lock()
            .map(|sessions| sessions.len())
            .unwrap_or(0);

        (symbol_count, session_count)
    }
} 