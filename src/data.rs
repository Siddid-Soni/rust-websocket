use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::time::interval;
use log::{info, warn, error};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StockData {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
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
}

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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_stock_data_from_csv_line() {
        let csv_line = "2024-01-01,100.0,105.0,95.0,102.0,1000000";
        let result = StockData::from_csv_line(csv_line, 0);
        
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.date, "2024-01-01");
        assert_eq!(data.open, 100.0);
        assert_eq!(data.high, 105.0);
        assert_eq!(data.low, 95.0);
        assert_eq!(data.close, 102.0);
        assert_eq!(data.volume, 1000000);
    }
    
    #[test]
    fn test_stock_data_invalid_csv() {
        let csv_line = "2024-01-01,100.0,105.0"; // Missing fields
        let result = StockData::from_csv_line(csv_line, 0);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected 6 fields"));
    }
    
    #[test]
    fn test_stock_data_to_json() {
        let data = StockData {
            date: "2024-01-01".to_string(),
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
            volume: 1000000,
        };
        
        let json = data.to_json();
        assert!(json.is_ok());
        assert!(json.unwrap().contains("2024-01-01"));
    }
} 