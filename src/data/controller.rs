use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};

use crate::data::{DataLoader, MultiSymbolDataBroadcaster, PubSubManager, StockData};
use crate::config::DATA_BROADCAST_INTERVAL_SECS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BroadcastState {
    Stopped,
    Running,
    Paused,
}

#[derive(Debug, Clone)]
pub enum BroadcastCommand {
    Start,
    Pause,
    Resume,
    Stop,
    Restart,
}

pub struct BroadcastController {
    state: Arc<Mutex<BroadcastState>>,
    pubsub_manager: Arc<PubSubManager>,
    cancel_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    loaded_data: Arc<Mutex<Option<HashMap<String, Vec<StockData>>>>>,
}

impl BroadcastController {
    pub fn new(pubsub_manager: Arc<PubSubManager>) -> Self {
        Self {
            state: Arc::new(Mutex::new(BroadcastState::Stopped)),
            pubsub_manager,
            cancel_handles: Arc::new(Mutex::new(Vec::new())),
            loaded_data: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_state(&self) -> BroadcastState {
        self.state.lock().unwrap().clone()
    }

    pub fn execute_command(&self, command: BroadcastCommand) -> Result<String, String> {
        let mut state = self.state.lock().unwrap();
        
        match (&*state, &command) {
            (BroadcastState::Stopped, BroadcastCommand::Start) => {
                drop(state); // Release lock before calling start_broadcasting
                self.start_broadcasting()
            }
            (BroadcastState::Running, BroadcastCommand::Pause) => {
                *state = BroadcastState::Paused;
                info!("📊 Broadcasting paused");
                Ok("Broadcasting paused successfully".to_string())
            }
            (BroadcastState::Paused, BroadcastCommand::Resume) => {
                *state = BroadcastState::Running;
                info!("📊 Broadcasting resumed");
                Ok("Broadcasting resumed successfully".to_string())
            }
            (_, BroadcastCommand::Stop) => {
                drop(state); // Release lock before calling stop_broadcasting
                self.stop_broadcasting()
            }
            (_, BroadcastCommand::Restart) => {
                drop(state); // Release lock before calling restart_broadcasting
                self.restart_broadcasting()
            }
            (current_state, cmd) => {
                Err(format!("Cannot execute {:?} while in state {:?}", cmd, current_state))
            }
        }
    }

    fn start_broadcasting(&self) -> Result<String, String> {
        // Load data first
        let symbol_data = match self.load_data() {
            Ok(data) => data,
            Err(e) => return Err(format!("Failed to load data: {}", e))
        };

        let symbol_count = symbol_data.len();
        let total_records = symbol_data.values().map(|data| data.len()).sum::<usize>();

        // Store loaded data
        *self.loaded_data.lock().unwrap() = Some(symbol_data.clone());

        // Clear any existing handles
        self.clear_handles();

        // Start broadcasting for each symbol
        let mut handles = Vec::new();
        for (symbol, data) in symbol_data {
            let handle = self.start_symbol_broadcast(symbol, data);
            handles.push(handle);
        }

        // Store handles
        *self.cancel_handles.lock().unwrap() = handles;

        // Update state
        *self.state.lock().unwrap() = BroadcastState::Running;

        info!("🚀 Started broadcasting for {} symbols with {} total records", symbol_count, total_records);
        Ok(format!("Broadcasting started for {} symbols with {} total records", symbol_count, total_records))
    }

    fn stop_broadcasting(&self) -> Result<String, String> {
        self.clear_handles();
        *self.state.lock().unwrap() = BroadcastState::Stopped;
        *self.loaded_data.lock().unwrap() = None;
        
        info!("🛑 Broadcasting stopped");
        Ok("Broadcasting stopped successfully".to_string())
    }

    fn restart_broadcasting(&self) -> Result<String, String> {
        // Stop first
        self.stop_broadcasting()?;
        // Then start
        self.start_broadcasting()
    }

    fn start_symbol_broadcast(&self, symbol: String, data: Vec<StockData>) -> JoinHandle<()> {
        let pubsub = self.pubsub_manager.clone();
        let state = self.state.clone();
        
        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(std::time::Duration::from_secs(DATA_BROADCAST_INTERVAL_SECS));
            let data_len = data.len();
            
            info!("Starting broadcast for symbol: {} ({} records)", symbol, data_len);
            
            for (i, stock_data) in data.into_iter().enumerate() {
                interval_timer.tick().await;
                
                // Check if we should continue broadcasting
                let should_continue = {
                    let current_state = state.lock().unwrap();
                    match *current_state {
                        BroadcastState::Stopped => {
                            info!("Stopping broadcast for symbol: {}", symbol);
                            false
                        }
                        BroadcastState::Paused => {
                            true // We'll handle pausing below
                        }
                        BroadcastState::Running => {
                            true
                        }
                    }
                };
                
                if !should_continue {
                    break;
                }
                
                // Handle paused state
                while {
                    let is_paused = state.lock().unwrap().clone();
                    matches!(is_paused, BroadcastState::Paused)
                } {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    
                    // Check if stopped while paused
                    let is_stopped = {
                        let current_state = state.lock().unwrap();
                        matches!(*current_state, BroadcastState::Stopped)
                    };
                    
                    if is_stopped {
                        break;
                    }
                }
                
                // Final check before broadcasting
                let should_broadcast = {
                    let current_state = state.lock().unwrap();
                    matches!(*current_state, BroadcastState::Running)
                };
                
                if !should_broadcast {
                    break;
                }
                
                let message = crate::data::StockMessage::new(symbol.clone(), stock_data);
                
                match message.to_json() {
                    Ok(json) => {
                        let subscriber_count = pubsub.broadcast_to_symbol(&symbol, &json)
                            .unwrap_or(0);
                        
                        if subscriber_count > 0 {
                            info!("Broadcasted {} data ({}/{}) to {} subscribers", 
                                  symbol, i + 1, data_len, subscriber_count);
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize stock message for {}: {}", symbol, e);
                    }
                }
            }
            
            info!("Completed broadcasting for symbol: {}", symbol);
        })
    }

    fn clear_handles(&self) {
        let mut handles = self.cancel_handles.lock().unwrap();
        for handle in handles.drain(..) {
            handle.abort();
        }
    }

    fn load_data(&self) -> Result<HashMap<String, Vec<StockData>>, Box<dyn std::error::Error>> {
        // Try to load multiple symbols from data directory
        match DataLoader::load_multiple_symbols("data") {
            Ok(symbol_data) => {
                info!("📊 Loaded {} symbols from directory", symbol_data.len());
                Ok(symbol_data)
            }
            Err(e) => {
                error!("Failed to load multiple symbols: {}", e);
                info!("Falling back to single file mode");
                
                // Fallback to single file broadcasting
                match DataLoader::load_from_csv("./data/NIFTY.csv") {
                    Ok(stock_data) => {
                        let mut symbol_data = HashMap::new();
                        symbol_data.insert("NIFTY".to_string(), stock_data);
                        info!("📊 Loaded fallback NIFTY data");
                        Ok(symbol_data)
                    }
                    Err(fallback_error) => {
                        Err(format!("Directory loading failed: {}. Fallback also failed: {}", e, fallback_error).into())
                    }
                }
            }
        }
    }

    pub fn get_status_info(&self) -> (BroadcastState, usize, usize) {
        let state = self.get_state();
        let (symbol_count, total_records) = if let Some(data) = self.loaded_data.lock().unwrap().as_ref() {
            (data.len(), data.values().map(|d| d.len()).sum())
        } else {
            (0, 0)
        };
        (state, symbol_count, total_records)
    }
} 