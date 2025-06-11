use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::time::interval;
use log::{info, warn, error};

use crate::data::pubsub::PubSubManager;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StockData {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StockMessage {
    pub symbol: String,
    pub data: StockData,
    pub timestamp: String,
}

impl StockData {
    pub fn from_csv_line(line: &str, line_num: usize) -> Result<Self, String> {
        let fields: Vec<&str> = line.split(',').collect();
        
        if fields.len() != 6 {
            return Err(format!(
                "Invalid CSV format at line {}: expected 6 fields, got {}", 
                line_num + 1, 
                fields.len()
            ));
        }
        
        Ok(StockData {
            date: fields[0].to_string(),
            open: fields[1].parse()
                .map_err(|e| format!("Invalid open price at line {}: {}", line_num + 1, e))?,
            high: fields[2].parse()
                .map_err(|e| format!("Invalid high price at line {}: {}", line_num + 1, e))?,
            low: fields[3].parse()
                .map_err(|e| format!("Invalid low price at line {}: {}", line_num + 1, e))?,
            close: fields[4].parse()
                .map_err(|e| format!("Invalid close price at line {}: {}", line_num + 1, e))?,
            volume: fields[5].parse()
                .map_err(|e| format!("Invalid volume at line {}: {}", line_num + 1, e))?,
        })
    }
    
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl StockMessage {
    pub fn new(symbol: String, data: StockData) -> Self {
        Self {
            symbol,
            data,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

pub struct DataLoader;

impl DataLoader {
    pub fn load_from_csv(file_path: &str) -> Result<Vec<StockData>, Box<dyn std::error::Error>> {
        let file = File::open(file_path)
            .map_err(|e| format!("Failed to open file {}: {}", file_path, e))?;
        
        let reader = BufReader::new(file);
        let mut stock_data: Vec<StockData> = Vec::new();
        let mut errors = Vec::new();

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result?;
            
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }
            
            match StockData::from_csv_line(&line, line_num) {
                Ok(data) => stock_data.push(data),
                Err(e) => {
                    error!("{}", e);
                    errors.push(e);
                    // Continue processing other lines
                }
            }
        }
        
        if !errors.is_empty() && stock_data.is_empty() {
            return Err(format!("Failed to load any valid data. {} errors encountered", errors.len()).into());
        }
        
        if !errors.is_empty() {
            warn!("Loaded {} records with {} errors", stock_data.len(), errors.len());
        } else {
            info!("Successfully loaded {} stock data records from {}", stock_data.len(), file_path);
        }
        
        Ok(stock_data)
    }

    pub fn load_multiple_symbols(data_dir: &str) -> Result<HashMap<String, Vec<StockData>>, Box<dyn std::error::Error>> {
        let mut symbol_data = HashMap::new();
        
        for entry in std::fs::read_dir(data_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension() == Some(std::ffi::OsStr::new("csv")) {
                if let Some(symbol) = path.file_stem().and_then(|s| s.to_str()) {
                    match Self::load_from_csv(&path.to_string_lossy()) {
                        Ok(data) => {
                            info!("Loaded {} records for symbol: {}", data.len(), symbol);
                            symbol_data.insert(symbol.to_string(), data);
                        }
                        Err(e) => {
                            error!("Failed to load data for symbol {}: {}", symbol, e);
                        }
                    }
                }
            }
        }
        
        info!("Successfully loaded data for {} symbols", symbol_data.len());
        Ok(symbol_data)
    }
}

// Original broadcaster for backwards compatibility
pub struct DataBroadcaster {
    data: Arc<Vec<StockData>>,
    interval_secs: u64,
}

impl DataBroadcaster {
    pub fn new(data: Vec<StockData>, interval_secs: u64) -> Self {
        Self {
            data: Arc::new(data),
            interval_secs,
        }
    }
    
    pub fn start_broadcasting(self, tx: broadcast::Sender<String>) {
        let data_len = self.data.len();
        
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(self.interval_secs));

            for (i, stock_data) in self.data.iter().enumerate() {
                interval_timer.tick().await;
                
                match stock_data.to_json() {
                    Ok(message) => {
                        if let Err(_) = tx.send(message.clone()) {
                            warn!("No active subscribers for data broadcast at record {}/{}", i + 1, data_len);
                        } else {
                            info!("Broadcasted stock data record {}/{}", i + 1, data_len);
                        }
                    }
                    Err(e) => {
                        error!("Failed to serialize stock data at record {}: {}", i + 1, e);
                    }
                }
            }

            // Send completion signal
            if let Err(_) = tx.send("done".to_string()) {
                warn!("Failed to send completion signal - no active subscribers");
            } else {
                info!("Data broadcasting completed. Sent completion signal.");
            }
        });
    }
    
    pub fn get_data_count(&self) -> usize {
        self.data.len()
    }
}

// New multi-symbol broadcaster for pub/sub
pub struct MultiSymbolDataBroadcaster {
    symbol_data: HashMap<String, Vec<StockData>>,
    pubsub: Arc<PubSubManager>,
    interval_secs: u64,
}

impl MultiSymbolDataBroadcaster {
    pub fn new(
        symbol_data: HashMap<String, Vec<StockData>>, 
        pubsub: Arc<PubSubManager>,
        interval_secs: u64
    ) -> Self {
        Self {
            symbol_data,
            pubsub,
            interval_secs,
        }
    }

    pub fn start_broadcasting(self) {
        for (symbol, data) in self.symbol_data {
            let pubsub = self.pubsub.clone();
            let interval_secs = self.interval_secs;
            
            tokio::spawn(async move {
                let mut interval_timer = interval(Duration::from_secs(interval_secs));
                let data_len = data.len();
                
                info!("Starting broadcast for symbol: {} ({} records)", symbol, data_len);
                
                for (i, stock_data) in data.into_iter().enumerate() {
                    interval_timer.tick().await;
                    
                    let message = StockMessage::new(symbol.clone(), stock_data);
                    
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
            });
        }
    }

    pub fn get_symbol_count(&self) -> usize {
        self.symbol_data.len()
    }

    pub fn get_total_records(&self) -> usize {
        self.symbol_data.values().map(|data| data.len()).sum()
    }
}
