use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::time::interval;
use log::{info, warn, error};
use chrono::{DateTime, Utc, NaiveDate, Duration as ChronoDuration, TimeZone};

use crate::data::pubsub::PubSubManager;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StockData {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scaled_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datetime: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StockMessage {
    pub symbol: String,
    pub data: StockData,
    pub timestamp: String,
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
    
    // Get the effective date for broadcasting (scaled date if available, otherwise original date)
    pub fn get_effective_date(&self) -> &str {
        self.data.scaled_date.as_ref().unwrap_or(&self.data.date)
    }
}

// Time period filtering helper
#[derive(Debug, Clone)]
pub enum TimePeriod {
    Minutes,
    Hour,
    Day,
}

impl TimePeriod {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "minutes" | "minute" | "min" | "m" => Some(TimePeriod::Minutes),
            "hour" | "hours" | "h" => Some(TimePeriod::Hour),
            "day" | "days" | "d" => Some(TimePeriod::Day),
            _ => None,
        }
    }
    
    pub fn filter_interval(&self) -> i64 {
        match self {
            TimePeriod::Minutes => 1,   // Every record (1 minute apart)
            TimePeriod::Hour => 60,     // Every 60th record (60 minutes = 1 hour apart)
            TimePeriod::Day => 375,     // Approximately 375 records per day (6.25 hours of market time)
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HistoricalDataResponse {
    pub success: bool,
    pub symbol: String,
    pub data: Vec<serde_json::Value>,
    pub total_records: usize,
    pub filtered_records: usize,
    pub date_range: Option<(String, String)>,
    pub time_period: Option<String>,
}

// Historical data manager for API access
pub struct HistoricalDataManager {
    pub all_symbol_data: HashMap<String, Vec<StockData>>,
    pub broadcast_start_date: Option<NaiveDate>, // The date when broadcasting starts (current date)
    pub historical_start_date: Option<NaiveDate>, // The first date in the last month's data
}

impl HistoricalDataManager {
    pub fn new(symbol_data: HashMap<String, Vec<StockData>>) -> Self {
        Self {
            all_symbol_data: symbol_data,
            broadcast_start_date: None,
            historical_start_date: None,
        }
    }

    // Set the date mapping for relative date queries
    pub fn set_date_mapping(&mut self, broadcast_start: NaiveDate, historical_start: NaiveDate) {
        self.broadcast_start_date = Some(broadcast_start);
        self.historical_start_date = Some(historical_start);
        info!("Date mapping set: current date {} maps to historical {}", 
              broadcast_start, historical_start);
    }

    // Convert a relative date query to the corresponding historical date
    fn map_relative_date(&self, query_date: &str) -> Option<String> {
        if let (Some(broadcast_start), Some(historical_start)) = 
            (self.broadcast_start_date, self.historical_start_date) {
            
            if let Ok(query_naive_date) = NaiveDate::parse_from_str(query_date, "%Y-%m-%d") {
                // Only allow queries for dates BEFORE or EQUAL to the broadcast start date
                if query_naive_date > broadcast_start {
                    return None; // Reject future dates
                }
                
                // Calculate the offset from broadcast start
                let days_offset = query_naive_date.signed_duration_since(broadcast_start).num_days();
                
                // Apply the same offset to historical start
                let mapped_date = historical_start + chrono::Duration::days(days_offset);
                
                return Some(mapped_date.format("%Y-%m-%d").to_string());
            }
        }
        
        // If no mapping is set, return the original date
        Some(query_date.to_string())
    }

    // Convert historical date back to scaled date for display
    fn map_historical_to_scaled_date(&self, historical_date: &str) -> String {
        if let (Some(broadcast_start), Some(historical_start)) = 
            (self.broadcast_start_date, self.historical_start_date) {
            
            if let Ok(historical_naive_date) = NaiveDate::parse_from_str(historical_date, "%Y-%m-%d") {
                // Calculate offset from historical start
                let days_offset = historical_naive_date.signed_duration_since(historical_start).num_days();
                
                // Apply offset to broadcast start+1 to get scaled date (start from today, not yesterday)
                let scaled_date = broadcast_start + chrono::Duration::days(1) + chrono::Duration::days(days_offset);
                
                return scaled_date.format("%Y-%m-%d").to_string();
            }
        }
        
        // If no mapping is set, return the original date
        historical_date.to_string()
    }

    pub fn get_symbol_data(
        &self, 
        symbol: &str, 
        limit: Option<usize>,
        from_date: Option<&str>,
        to_date: Option<&str>,
        time_period: Option<&str>,
    ) -> HistoricalDataResponse {
        match self.all_symbol_data.get(symbol) {
            Some(data) => {
                let mut filtered_data = data.clone();
                
                // If no date range is provided, filter to recent scalable data by default
                if from_date.is_none() && to_date.is_none() {
                    if let Some(historical_start) = self.historical_start_date {
                        let historical_start_str = historical_start.format("%Y-%m-%d").to_string();
                        filtered_data = self.filter_from_date(filtered_data, &historical_start_str);
                    }
                }
                
                // Apply relative date mapping for date range filtering
                let mapped_from_date = from_date.and_then(|d| self.map_relative_date(d));
                let mapped_to_date = to_date.and_then(|d| self.map_relative_date(d));
                
                // If date mapping failed (future date requested), return empty result
                if from_date.is_some() && mapped_from_date.is_none() {
                    return HistoricalDataResponse {
                        success: false,
                        symbol: symbol.to_string(),
                        data: Vec::new(),
                        total_records: data.len(),
                        filtered_records: 0,
                        date_range: None,
                        time_period: time_period.map(|s| s.to_string()),
                    };
                }
                
                if to_date.is_some() && mapped_to_date.is_none() {
                    return HistoricalDataResponse {
                        success: false,
                        symbol: symbol.to_string(),
                        data: Vec::new(),
                        total_records: data.len(),
                        filtered_records: 0,
                        date_range: None,
                        time_period: time_period.map(|s| s.to_string()),
                    };
                }
                
                if let (Some(from), Some(to)) = (mapped_from_date.as_deref(), mapped_to_date.as_deref()) {
                    filtered_data = self.filter_by_date_range(filtered_data, from, to);
                } else if let Some(from) = mapped_from_date.as_deref() {
                    filtered_data = self.filter_from_date(filtered_data, from);
                } else if let Some(to) = mapped_to_date.as_deref() {
                    filtered_data = self.filter_to_date(filtered_data, to);
                }
                
                // Apply time period filtering
                if let Some(period_str) = time_period {
                    if let Some(period) = TimePeriod::from_string(period_str) {
                        filtered_data = self.filter_by_time_period(filtered_data, &period);
                    }
                }
                
                // Apply limit if specified
                if let Some(limit_val) = limit {
                    filtered_data.truncate(limit_val);
                }

                // Convert all dates to scaled dates for display
                let mut scaled_data = Vec::new();
                let mut current_date: Option<String> = None;
                let mut records_in_current_day = 0;
                
                // Determine time increment based on time period
                let time_increment_minutes = if let Some(period_str) = time_period {
                    match period_str.to_lowercase().as_str() {
                        "hour" | "hours" | "h" => 60,  // 1 hour = 60 minutes
                        "day" | "days" | "d" => 24 * 60,  // 1 day = 1440 minutes (but this is handled separately)
                        _ => 1,  // minutes or default: 1 minute
                    }
                } else {
                    1  // default: 1 minute
                };
                
                for mut stock_data in filtered_data {
                    stock_data.scaled_date = Some(self.map_historical_to_scaled_date(&stock_data.date));
                    
                    // Generate a datetime that matches the scaled date with proper time progression
                    if let Some(scaled_date_str) = &stock_data.scaled_date {
                        if let Ok(scaled_date) = NaiveDate::parse_from_str(scaled_date_str, "%Y-%m-%d") {
                            // Check if we're on a new day
                            if current_date.as_ref() != Some(scaled_date_str) {
                                current_date = Some(scaled_date_str.clone());
                                records_in_current_day = 0;
                            }
                            
                            // Calculate time progression within the day
                            // Start at market open (9:15 AM) and increment based on time period
                            let market_open_time = chrono::NaiveTime::from_hms_opt(9, 15, 0).unwrap();
                            let minutes_to_add = records_in_current_day * time_increment_minutes;
                            
                            // Add minutes to market open time
                            let current_time = market_open_time + chrono::Duration::minutes(minutes_to_add as i64);
                            let market_datetime = scaled_date.and_time(current_time);
                            
                            // Format as "YYYY-MM-DD HH:MM:SS"
                            stock_data.datetime = Some(market_datetime.format("%Y-%m-%d %H:%M:%S").to_string());
                            
                            records_in_current_day += 1;
                        }
                    }
                    
                    scaled_data.push(stock_data);
                }

                // Convert to API response format
                let api_data: Vec<serde_json::Value> = scaled_data.iter()
                    .map(|data| data.to_api_response())
                    .collect();

                // Get date range (show scaled dates)
                let date_range = if !scaled_data.is_empty() {
                    let first_scaled = scaled_data.first().unwrap().scaled_date.as_ref().unwrap().clone();
                    let last_scaled = scaled_data.last().unwrap().scaled_date.as_ref().unwrap().clone();
                    Some((first_scaled, last_scaled))
                } else {
                    None
                };

                // Log the relative date mapping if it was applied
                if let (Some(orig_from), Some(mapped_from)) = (from_date, mapped_from_date.as_deref()) {
                    if orig_from != mapped_from {
                        info!("Mapped relative date query: {} -> {} for symbol {}", 
                              orig_from, mapped_from, symbol);
                    }
                }

                HistoricalDataResponse {
                    success: true,
                    symbol: symbol.to_string(),
                    data: api_data,
                    total_records: data.len(),
                    filtered_records: scaled_data.len(),
                    date_range,
                    time_period: time_period.map(|s| s.to_string()),
                }
            }
            None => {
                HistoricalDataResponse {
                    success: false,
                    symbol: symbol.to_string(),
                    data: Vec::new(),
                    total_records: 0,
                    filtered_records: 0,
                    date_range: None,
                    time_period: time_period.map(|s| s.to_string()),
                }
            }
        }
    }
    
    fn filter_by_date_range(&self, mut data: Vec<StockData>, from_date: &str, to_date: &str) -> Vec<StockData> {
        if let (Ok(from), Ok(to)) = (
            NaiveDate::parse_from_str(from_date, "%Y-%m-%d"),
            NaiveDate::parse_from_str(to_date, "%Y-%m-%d")
        ) {
            data.retain(|record| {
                if let Ok(record_date) = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d") {
                    record_date >= from && record_date <= to
                } else {
                    false
                }
            });
        }
        data
    }
    
    fn filter_from_date(&self, mut data: Vec<StockData>, from_date: &str) -> Vec<StockData> {
        if let Ok(from) = NaiveDate::parse_from_str(from_date, "%Y-%m-%d") {
            data.retain(|record| {
                if let Ok(record_date) = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d") {
                    record_date >= from
                } else {
                    false
                }
            });
        }
        data
    }
    
    fn filter_to_date(&self, mut data: Vec<StockData>, to_date: &str) -> Vec<StockData> {
        if let Ok(to) = NaiveDate::parse_from_str(to_date, "%Y-%m-%d") {
            data.retain(|record| {
                if let Ok(record_date) = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d") {
                    record_date <= to
                } else {
                    false
                }
            });
        }
        data
    }
    
    fn filter_by_time_period(&self, data: Vec<StockData>, period: &TimePeriod) -> Vec<StockData> {
        let interval = period.filter_interval();
        
        match period {
            TimePeriod::Day => {
                // For daily data, take one record per day (assuming data is sorted)
                let mut result = Vec::new();
                let mut last_date: Option<String> = None;
                
                for record in data {
                    let current_date = record.date.clone();
                    if last_date.as_ref() != Some(&current_date) {
                        result.push(record);
                        last_date = Some(current_date);
                    }
                }
                result
            }
            _ => {
                // For other periods, sample at intervals
                data.into_iter()
                    .enumerate()
                    .filter(|(i, _)| (*i as i64) % interval == 0)
                    .map(|(_, record)| record)
                    .collect()
            }
        }
    }

    pub fn get_available_symbols(&self) -> Vec<String> {
        self.all_symbol_data.keys().cloned().collect()
    }

    pub fn get_symbols_summary(&self) -> HashMap<String, usize> {
        self.all_symbol_data.iter()
            .map(|(symbol, data)| (symbol.clone(), data.len()))
            .collect()
    }
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
        
        // Extract date part from datetime string (before the space)
        let date_str = {
            let datetime_str = fields[0].trim();
            if let Some(space_pos) = datetime_str.find(' ') {
                &datetime_str[..space_pos]
            } else {
                datetime_str
            }
        };

        // Generate a timestamp based on the date (assuming market open at 9:15 AM IST)
        let datetime = match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Ok(date) => {
                // Create a datetime at 9:15 AM (market open time)
                let market_open_time = chrono::NaiveTime::from_hms_opt(9, 15, 0).unwrap();
                let market_open_datetime = date.and_time(market_open_time);
                
                // Format as "YYYY-MM-DD HH:MM:SS"
                Some(market_open_datetime.format("%Y-%m-%d %H:%M:%S").to_string())
            },
            Err(_) => None,
        };
        
        Ok(StockData {
            date: date_str.to_string(),
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
            scaled_date: None,
            original_date: None,
            datetime,
        })
    }
    
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    // Check if this data point is within the last month from the latest date in dataset
    pub fn is_within_last_month(&self, latest_date: &NaiveDate) -> bool {
        if let Ok(data_date) = NaiveDate::parse_from_str(&self.date, "%Y-%m-%d") {
            let one_month_ago = *latest_date - chrono::Duration::days(30);
            data_date >= one_month_ago && data_date <= *latest_date
        } else {
            false
        }
    }

    // Check if this data point is within the last two months from the latest date in dataset
    pub fn is_within_last_two_months(&self, latest_date: &NaiveDate) -> bool {
        if let Ok(data_date) = NaiveDate::parse_from_str(&self.date, "%Y-%m-%d") {
            let two_months_ago = *latest_date - chrono::Duration::days(60);
            data_date >= two_months_ago && data_date <= *latest_date
        } else {
            false
        }
    }

    // Get the display date (scaled if available, otherwise original)
    pub fn get_display_date(&self) -> &str {
        self.scaled_date.as_ref().unwrap_or(&self.date)
    }
    
    // Custom serialization to show scaled datetime only in API responses
    pub fn to_api_response(&self) -> serde_json::Value {
        serde_json::json!({
            "datetime": self.datetime.as_ref().unwrap_or(&chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
            "open": self.open,
            "high": self.high,
            "low": self.low,
            "close": self.close,
            "volume": self.volume
        })
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
        let mut is_first_line = true;
        let mut current_date: Option<String> = None;
        let mut records_in_current_day = 0;

        for (line_num, line_result) in reader.lines().enumerate() {
            let line = line_result?;
            
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }
            
            // Skip header row (first non-empty line)
            if is_first_line {
                is_first_line = false;
                // Check if this looks like a header (contains "date", "open", etc.)
                let line_lower = line.to_lowercase();
                if line_lower.contains("date") && line_lower.contains("open") && line_lower.contains("close") {
                    info!("Skipping header row: {}", line);
                    continue;
                }
            }
            
            match StockData::from_csv_line(&line, line_num) {
                Ok(mut data) => {
                    // Add proper time progression for intraday data
                    if let Some(datetime_str) = &data.datetime {
                        // Extract just the date part for comparison
                        let date_part = data.date.clone();
                        
                        // Check if we're on a new day
                        if current_date.as_ref() != Some(&date_part) {
                            current_date = Some(date_part.clone());
                            records_in_current_day = 0;
                        }
                        
                        // Regenerate datetime with proper minute progression
                        if let Ok(date) = NaiveDate::parse_from_str(&date_part, "%Y-%m-%d") {
                            let market_open_time = chrono::NaiveTime::from_hms_opt(9, 15, 0).unwrap();
                            let current_time = market_open_time + chrono::Duration::minutes(records_in_current_day as i64);
                            let market_datetime = date.and_time(current_time);
                            
                            data.datetime = Some(market_datetime.format("%Y-%m-%d %H:%M:%S").to_string());
                            records_in_current_day += 1;
                        }
                    }
                    
                    stock_data.push(data);
                },
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

    // Prepare broadcast data with date scaling
    pub fn prepare_broadcast_data(
        symbol_data: HashMap<String, Vec<StockData>>,
        time_scale_factor: f64, // Used for scaling time intervals, not dates
    ) -> Result<(HashMap<String, Vec<StockData>>, HashMap<String, Vec<StockData>>), Box<dyn std::error::Error>> {
        let mut broadcast_data = HashMap::new();
        let all_data = symbol_data.clone(); // Keep all data for historical API

        // Get current date
        let current_date = chrono::Utc::now().date_naive();
        
        for (symbol, mut data) in symbol_data {
            // Sort data by date
            data.sort_by(|a, b| a.date.cmp(&b.date));

            if data.is_empty() {
                continue;
            }

            // Find the earliest and latest dates in the dataset
            let earliest_date = NaiveDate::parse_from_str(&data.first().unwrap().date, "%Y-%m-%d")
                .map_err(|e| format!("Failed to parse earliest date for {}: {}", symbol, e))?;
            let latest_date = NaiveDate::parse_from_str(&data.last().unwrap().date, "%Y-%m-%d")
                .map_err(|e| format!("Failed to parse latest date for {}: {}", symbol, e))?;

            // Calculate total duration of historical data
            let historical_duration = latest_date.signed_duration_since(earliest_date).num_days();
            
            info!("Historical data for {}: {} to {} ({} days)", 
                  symbol, earliest_date, latest_date, historical_duration);

            // Filter for last two months data first
            let last_two_months_data: Vec<StockData> = data.into_iter()
                .filter(|stock_data| stock_data.is_within_last_two_months(&latest_date))
                .collect();

            if last_two_months_data.is_empty() {
                warn!("No data in last two months for symbol: {}", symbol);
                continue;
            }

            // Get the date range for last two months data
            let last_two_months_start = NaiveDate::parse_from_str(&last_two_months_data.first().unwrap().date, "%Y-%m-%d")
                .map_err(|e| format!("Failed to parse last two months start date for {}: {}", symbol, e))?;
            let last_two_months_end = NaiveDate::parse_from_str(&last_two_months_data.last().unwrap().date, "%Y-%m-%d")
                .map_err(|e| format!("Failed to parse last two months end date for {}: {}", symbol, e))?;
            let last_two_months_duration = last_two_months_end.signed_duration_since(last_two_months_start).num_days();

            // Apply date scaling to last two months data
            // Map current_date to last_two_months_start, and scale the rest proportionally
            let mut scaled_data = Vec::new();
            for stock_data in last_two_months_data {
                let original_date = NaiveDate::parse_from_str(&stock_data.date, "%Y-%m-%d")
                    .map_err(|e| format!("Failed to parse date {} for scaling: {}", stock_data.date, e))?;
                
                // Calculate how far this date is from the start of last two months data (as a ratio)
                let days_from_last_two_months_start = original_date.signed_duration_since(last_two_months_start).num_days();
                
                // Map current date to last_two_months_start, and scale proportionally from there
                // So first record starts today, and subsequent records follow the time progression
                let scaled_date = current_date + chrono::Duration::days(days_from_last_two_months_start);
                
                // Generate scaled timestamp for the scaled date
                let scaled_datetime = {
                    let market_open_time = chrono::NaiveTime::from_hms_opt(9, 15, 0).unwrap();
                    let market_open_datetime = scaled_date.and_time(market_open_time);
                    market_open_datetime.format("%Y-%m-%d %H:%M:%S").to_string()
                };
                
                let mut scaled_stock_data = stock_data.clone();
                scaled_stock_data.scaled_date = Some(scaled_date.format("%Y-%m-%d").to_string());
                // Update datetime with scaled datetime for consistency
                scaled_stock_data.datetime = Some(scaled_datetime);
                
                scaled_data.push(scaled_stock_data);
            }

            // Sort scaled data by scaled_date for proper broadcasting order
            scaled_data.sort_by(|a, b| {
                let a_scaled = a.scaled_date.as_ref().unwrap();
                let b_scaled = b.scaled_date.as_ref().unwrap();
                a_scaled.cmp(b_scaled)
            });

            info!("Prepared {} records for broadcasting (last two months, date-scaled) for symbol: {}", 
                  scaled_data.len(), symbol);
            
            if !scaled_data.is_empty() {
                let first_scaled = scaled_data.first().unwrap().scaled_date.as_ref().unwrap();
                let last_scaled = scaled_data.last().unwrap().scaled_date.as_ref().unwrap();
                info!("Date scaling for {}: {} -> {} to {} -> {} (starting from current date)", 
                      symbol, 
                      scaled_data.first().unwrap().date, first_scaled,
                      scaled_data.last().unwrap().date, last_scaled);
            }
            
            broadcast_data.insert(symbol, scaled_data);
        }

        Ok((broadcast_data, all_data))
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

// New multi-symbol broadcaster for pub/sub with date-based timing
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
                let data_len = data.len();
                
                info!("Starting broadcast for symbol: {} ({} records)", symbol, data_len);
                
                // Calculate timing based on scaled dates if available
                let start_time = chrono::Utc::now();
                let mut previous_scaled_date: Option<NaiveDate> = None;
                
                for (i, stock_data) in data.into_iter().enumerate() {
                    let message = StockMessage::new(symbol.clone(), stock_data.clone());
                    
                    // Calculate wait time based on date progression
                    let wait_duration = if let Some(scaled_date_str) = &stock_data.scaled_date {
                        if let Ok(current_scaled_date) = NaiveDate::parse_from_str(scaled_date_str, "%Y-%m-%d") {
                            if let Some(prev_date) = previous_scaled_date {
                                let days_diff = current_scaled_date.signed_duration_since(prev_date).num_days();
                                // Convert days to seconds based on scaling factor
                                // 1 day in historical data = interval_secs in real time
                                Duration::from_secs((days_diff.max(0) as u64) * interval_secs)
                            } else {
                                Duration::from_secs(0) // First record, no wait
                            }
                        } else {
                            Duration::from_secs(interval_secs) // Fallback to regular interval
                        }
                    } else {
                        Duration::from_secs(interval_secs) // No scaled date, use regular interval
                    };
                    
                    // Update previous date for next iteration
                    if let Some(scaled_date_str) = &stock_data.scaled_date {
                        if let Ok(current_scaled_date) = NaiveDate::parse_from_str(scaled_date_str, "%Y-%m-%d") {
                            previous_scaled_date = Some(current_scaled_date);
                        }
                    }
                    
                    // Wait before broadcasting
                    if wait_duration > Duration::from_secs(0) {
                        tokio::time::sleep(wait_duration).await;
                    }
                    
                    match message.to_json() {
                        Ok(json) => {
                            let subscriber_count = pubsub.broadcast_to_symbol(&symbol, &json)
                                .unwrap_or(0);
                            
                            if subscriber_count > 0 {
                                let effective_date = message.get_effective_date();
                                info!("Broadcasted {} data ({}/{}) [{}] to {} subscribers", 
                                      symbol, i + 1, data_len, effective_date, subscriber_count);
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
