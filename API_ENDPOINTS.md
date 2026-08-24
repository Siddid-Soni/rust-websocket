# NSE Socket API Endpoints

This document describes all available REST API endpoints for the NSE Socket server.

## Base URL
- **Development**: `http://localhost:3000/api`
- **WebSocket**: `ws://localhost:8080`

## Authentication
Most endpoints require JWT authentication. Include the token in the Authorization header:
```
Authorization: Bearer <your-jwt-token>
```

## Endpoints

### Authentication

#### POST /api/login
Generate a JWT token for API access.

**Request Body:**
```json
{
  "username": "your_username"
}
```

**Response:**
```json
{
  "success": true,
  "message": "Token generated successfully",
  "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
  "user_id": "your_username",
  "permissions": ["user"]
}
```

### Historical Data API 📊

#### GET /api/historical
Get summary of all available symbols and their record counts.

**Headers:** `Authorization: Bearer <token>`

**Response:**
```json
{
  "success": true,
  "symbols": ["NIFTY", "BANKNIFTY", "RELIANCE"],
  "symbol_counts": {
    "NIFTY": 5000,
    "BANKNIFTY": 4800,
    "RELIANCE": 5200
  },
  "total_symbols": 3,
  "total_records": 15000
}
```

#### GET /api/historical/{symbol}
Get historical data for a specific symbol with advanced filtering options.

**Headers:** `Authorization: Bearer <token>`

**Query Parameters:**
- `limit` (optional): Maximum number of records to return
- `from_date` (optional): Start date filter (YYYY-MM-DD format)
- `to_date` (optional): End date filter (YYYY-MM-DD format)  
- `time_period` (optional): Time period filtering
  - `seconds` or `s`: Every record (no filtering)
  - `minutes` or `min` or `m`: Every 60th record
  - `hour` or `hours` or `h`: Every 3600th record
  - `day` or `days` or `d`: One record per unique date

**Examples:**
```bash
# Get last 100 records
GET /api/historical/NIFTY?limit=100

# Get January 2024 data
GET /api/historical/NIFTY?from_date=2024-01-01&to_date=2024-01-31

# Get daily data for last 30 days
GET /api/historical/NIFTY?time_period=day&limit=30

# Get hourly data for specific date range
GET /api/historical/NIFTY?from_date=2024-01-01&to_date=2024-01-15&time_period=hour

# Combined filtering: January daily data, max 10 records
GET /api/historical/NIFTY?from_date=2024-01-01&to_date=2024-01-31&time_period=day&limit=10
```

**Response:**
```json
{
  "success": true,
  "symbol": "NIFTY",
  "data": [
    {
      "date": "2024-01-01",
      "open": 21725.70,
      "high": 21801.45,
      "low": 21692.95,
      "close": 21731.40,
      "volume": 142789654,
      "scaled_timestamp": "2024-12-20T10:30:00Z"
    }
  ],
  "total_records": 5000,
  "filtered_records": 31,
  "date_range": ["2024-01-01", "2024-01-31"],
  "time_period": "day"
}
```

### Data Broadcasting Control (Admin Only) 📡

#### POST /api/start-broadcast
Start real-time data broadcasting for the last month's data.

**Headers:** `Authorization: Bearer <admin-token>`

**Response:**
```json
{
  "success": true,
  "message": "Broadcasting started for 3 symbols with 2790 total records"
}
```

#### POST /api/pause-broadcast
Pause the current broadcasting without stopping.

**Headers:** `Authorization: Bearer <admin-token>`

**Response:**
```json
{
  "success": true,
  "message": "Broadcasting paused successfully"
}
```

#### POST /api/resume-broadcast
Resume paused broadcasting.

**Headers:** `Authorization: Bearer <admin-token>`

**Response:**
```json
{
  "success": true,
  "message": "Broadcasting resumed successfully"
}
```

#### POST /api/stop-broadcast
Stop broadcasting completely.

**Headers:** `Authorization: Bearer <admin-token>`

**Response:**
```json
{
  "success": true,
  "message": "Broadcasting stopped successfully"
}
```

#### POST /api/restart-broadcast
Restart broadcasting from the beginning.

**Headers:** `Authorization: Bearer <admin-token>`

**Response:**
```json
{
  "success": true,
  "message": "Broadcasting started for 3 symbols with 2790 total records"
}
```

#### GET /api/broadcast-status
Get current broadcasting status and statistics.

**Headers:** `Authorization: Bearer <admin-token>`

**Response:**
```json
{
  "success": true,
  "state": "Running",
  "symbol_count": 3,
  "total_records": 2790,
  "message": "Broadcasting is Running with 3 symbols and 2790 total records"
}
```

### Trading Orders 💹

#### POST /api/orders
Place a new trading order.

**Headers:** `Authorization: Bearer <token>`

**Request Body:**
```json
{
  "symbol": "NIFTY",
  "order_type": "Market",
  "side": "Buy",
  "quantity": 50,
  "price": 21750.00
}
```

**Response:**
```json
{
  "success": true,
  "message": "Order placed successfully",
  "order": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "symbol": "NIFTY",
    "order_type": "Market",
    "side": "Buy",
    "quantity": 50,
    "price": 21750.00,
    "status": "Pending",
    "user_id": "test_user",
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

#### GET /api/orders
Get user's trading orders with optional filtering.

**Headers:** `Authorization: Bearer <token>`

**Query Parameters:**
- `symbol` (optional): Filter by symbol
- `status` (optional): Filter by status (pending, filled, cancelled, rejected)
- `limit` (optional): Maximum number of orders to return

**Response:**
```json
{
  "success": true,
  "orders": [...],
  "total": 15
}
```

#### GET /api/orders/{order_id}
Get details of a specific order.

**Headers:** `Authorization: Bearer <token>`

**Response:**
```json
{
  "success": true,
  "message": "Order retrieved successfully",
  "order": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "symbol": "NIFTY",
    "order_type": "Market",
    "side": "Buy",
    "quantity": 50,
    "price": 21750.00,
    "status": "Filled",
    "user_id": "test_user",
    "created_at": "2024-01-15T10:30:00Z"
  }
}
```

#### DELETE /api/orders/{order_id}
Cancel a pending order.

**Headers:** `Authorization: Bearer <token>`

**Response:**
```json
{
  "success": true,
  "message": "Order cancelled successfully",
  "order": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "status": "Cancelled"
  }
}
```

### System Health

#### GET /api/health
Check API server health status.

**Response:**
```json
{
  "status": "healthy",
  "service": "nse_socket_api",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Key Features 🚀

### 1. Last Month Data Broadcasting
- Only the last 30 days of data from each symbol is used for real-time broadcasting
- Full historical data remains available via API endpoints
- Configurable time scaling for realistic simulation

### 2. Date Scaling (Updated)
- **Only dates are scaled, time remains current**
- Default scale factor: 0.01 (100x faster date progression)
- File dates map to current date + scaled offset
- Configurable via `TIME_SCALE_FACTOR` environment variable

### 3. Advanced Data Filtering
- **Date Range**: Filter by start and end dates
- **Time Period**: Sample data at different intervals (seconds, minutes, hours, days)
- **Combined Filtering**: Use multiple filters together
- **Limit**: Control maximum number of records returned

### 4. Real-time Broadcasting
- WebSocket-based real-time data streaming
- Pub/Sub pattern for efficient delivery
- Scaled timestamps for realistic timing
- Admin controls for start/stop/pause/resume

## Configuration

### Environment Variables
```bash
# Server Configuration
BIND_ADDRESS=0.0.0.0:8080          # WebSocket server address
API_BIND_ADDRESS=0.0.0.0:3000      # HTTP API server address
RUST_LOG=info                       # Log level

# Data Configuration  
DATA_DIR=./data                     # Directory containing CSV files
TIME_SCALE_FACTOR=0.01             # Date scaling factor (0.01 = 100x faster)
BROADCAST_INTERVAL_SECS=1          # Fallback interval for broadcasting

# JWT Configuration
JWT_SECRET=your-secret-key          # JWT signing secret (min 32 chars)
```

### Time Scale Examples
- `TIME_SCALE_FACTOR=1.0`: Real-time date progression
- `TIME_SCALE_FACTOR=0.1`: 10x faster date progression  
- `TIME_SCALE_FACTOR=0.01`: 100x faster date progression (default)
- `TIME_SCALE_FACTOR=0.001`: 1000x faster date progression

## WebSocket API

### Connection
```
ws://localhost:8080/ws              # Regular users
ws://localhost:8080/admin           # Admin users (order events)
```

### Authentication
Include JWT token in:
- Header: `Authorization: Bearer <token>`
- Query parameter: `?token=<token>`

### Message Format
```json
{
  "type": "subscribe",
  "symbol": "NIFTY"
}
```

## Error Handling

All endpoints return standardized error responses:

```json
{
  "success": false,
  "message": "Error description",
  "error_code": "INVALID_REQUEST"
}
```

## Rate Limiting

- API endpoints: No rate limiting currently
- WebSocket: Connection-based limits
- Historical data: Large queries may take longer to process

## Data Format

### Stock Data Structure
```json
{
  "date": "2024-01-15",              # Original file date (YYYY-MM-DD)
  "open": 21725.70,                  # Opening price
  "high": 21801.45,                  # High price
  "low": 21692.95,                   # Low price  
  "close": 21731.40,                 # Closing price
  "volume": 142789654,               # Volume
  "scaled_timestamp": "2024-12-20T10:30:00Z"  # Scaled timestamp for broadcasting
}
```

The `scaled_timestamp` field is added during data preparation and represents when this data point should be broadcast in the scaled timeline. 