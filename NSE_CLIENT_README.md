# NSE Socket Client Library

A Python client library for the NSE Socket server that provides a clean, easy-to-use API similar to popular trading libraries like Breeze API.

## 🚀 Quick Start

### Installation

```bash
# Install required dependencies
pip install websocket-client requests
```

### Basic Usage

```python
from nse_client import NSEClient, create_client

# Method 1: Quick setup (auto-loads token)
client = create_client()

# Method 2: Explicit setup (like breeze pattern)
client = NSEClient(
    ws_uri="ws://localhost:8080",
    api_uri="http://localhost:3000", 
    token="your-jwt-token"
)

# Set up callbacks
def on_ticks(ticks):
    print(f"📊 {ticks['symbol']}: ₹{ticks['data']['close']:.2f}")

client.on_ticks = on_ticks

# Connect and subscribe
client.ws_connect()
client.subscribe_feed("NIFTY")

# Get historical data
historical_data = client.get_historical_data("NIFTY", limit=100)
daily_data = client.get_historical_data("NIFTY", time_period="day", limit=30)

# Place orders
order = client.place_order("NIFTY", "buy", "market", 100)

# Cleanup
client.ws_disconnect()
```

## 📡 Real-Time Data Streaming

### WebSocket Connection

```python
from nse_client import create_client

# Create client
client = create_client()

# Define callbacks
def on_ticks(ticks):
    """Receive real-time stock data"""
    symbol = ticks["symbol"]
    data = ticks["data"]
    
    print(f"📊 {symbol}: Close=₹{data['close']:.2f}, Volume={data['volume']:,}")

def on_connect():
    """Called when WebSocket connects"""
    print("✅ Connected to NSE server!")

def on_disconnect():
    """Called when WebSocket disconnects"""
    print("❌ Disconnected from server")

# Assign callbacks
client.on_ticks = on_ticks
client.on_connect = on_connect
client.on_disconnect = on_disconnect

# Connect to WebSocket
if client.ws_connect():
    print("🚀 Connected successfully!")
else:
    print("❌ Connection failed")
```

### Feed Subscription

```python
# Subscribe to a symbol
client.subscribe_feed("NIFTY")

# Switch to another symbol (auto-unsubscribes from previous)
client.subscribe_feed("INDIGO")

# Explicit unsubscribe
client.unsubscribe_feed("INDIGO")

# Check current subscription
current_symbol = client.get_current_symbol()
print(f"Currently subscribed to: {current_symbol}")
```

### Data Format

The `on_ticks` callback receives data in this format:

```python
{
    "symbol": "NIFTY",
    "data": {
        "date": "2024-01-15",
        "open": 21500.0,
        "high": 21650.0,
        "low": 21480.0,
        "close": 21620.0,
        "volume": 1500000
    },
    "timestamp": "2024-01-15T10:30:00Z",
    "datetime": datetime.now()  # Python datetime object
}
```

## 📊 Historical Data API

The NSE client now provides comprehensive historical data access with advanced filtering, date scaling, and time period sampling capabilities.

### Getting Historical Data Summary

```python
# Get overview of available data
summary = client.get_historical_summary()
if summary:
    print(f"Available symbols: {summary['symbols']}")
    print(f"Total records: {summary['total_records']}")
    print(f"Symbol counts: {summary['symbol_counts']}")

# Quick way to get available symbols
symbols = client.get_available_symbols()
print(f"Available: {symbols}")
```

### Basic Historical Data Queries

```python
# Get last 100 records for a symbol
data = client.get_historical_data("NIFTY", limit=100)
if data and data['success']:
    records = data['data']
    print(f"Retrieved {len(records)} records")
    print(f"Date range: {data['date_range']}")

# Simple interface (returns just the data array)
records = client.get_historical_data_simple("NIFTY", limit=50)
for record in records:
    print(f"{record['date']}: Close ₹{record['close']}")
```

### Clean Historical Data (Essential Fields Only)

For applications that need only the core OHLCV data without any metadata:

```python
# Get clean data with only essential fields: date, open, high, low, close, volume
clean_data = client.get_historical_data_clean("NIFTY", limit=10)

# Returns exactly: [{"date": "2025-06-21", "open": 21500.0, "high": 21650.0, "low": 21480.0, "close": 21620.0, "volume": 1500000}, ...]
for record in clean_data:
    print(f"{record['date']}: O={record['open']:.2f} H={record['high']:.2f} L={record['low']:.2f} C={record['close']:.2f} V={record['volume']:,}")

# Clean data with filtering
clean_data = client.get_historical_data_clean(
    "INDIGO",
    time_period="minutes",
    from_date="2025-06-19",
    to_date="2025-06-19",
    limit=10
)
```

### Date Range Filtering with Date Scaling

The system supports **relative date queries** that work with the server's date scaling feature:

```python
from datetime import date, timedelta

# Query for yesterday's data (relative to current broadcast timeline)
yesterday = date.today() - timedelta(days=1)
data = client.get_historical_data("NIFTY", from_date=yesterday)

# Query for last week's data
week_ago = date.today() - timedelta(days=7)
data = client.get_historical_data(
    "NIFTY",
    from_date=week_ago,
    to_date=yesterday,
    limit=100
)

# Date range with string format
data = client.get_historical_data(
    "NIFTY",
    from_date="2025-06-15",  # These are scaled dates
    to_date="2025-06-20",
    limit=50
)
```

### Time Period Filtering

Sample data at different time intervals:

```python
# Daily data (one record per day)
daily_data = client.get_historical_data(
    "NIFTY",
    time_period="day",
    limit=30
)

# Hourly sampling 
hourly_data = client.get_historical_data(
    "NIFTY", 
    time_period="hour",
    limit=24
)

# Minute-level sampling
minute_data = client.get_historical_data(
    "NIFTY",
    time_period="minutes",  # Can also use "min" or "m"
    limit=60
)
```

### Combined Filtering

Use multiple filters together for precise data selection:

```python
from datetime import date, timedelta

# Get daily data for the last 2 weeks
two_weeks_ago = date.today() - timedelta(days=14)
yesterday = date.today() - timedelta(days=1)

data = client.get_historical_data(
    "NIFTY",
    from_date=two_weeks_ago,
    to_date=yesterday,
    time_period="day",
    limit=10
)

if data and data['success']:
    print(f"Retrieved {data['filtered_records']} daily records")
    print(f"Date range: {data['date_range']}")
    for record in data['data']:
        print(f"{record['date']}: ₹{record['close']:.2f} (original: {record['original_date']})")
```

### Historical Data Response Format

```python
{
    "success": true,
    "symbol": "NIFTY",
    "data": [
        {
            "date": "2025-06-15",         # Scaled date (for display)
            "open": 21725.70,
            "high": 21801.45,
            "low": 21692.95,
            "close": 21731.40,
            "volume": 142789654,
            "original_date": "2024-10-04"  # Original historical date
        }
    ],
    "total_records": 5000,              # Total records in symbol
    "filtered_records": 10,             # Records after filtering
    "date_range": ["2025-06-15", "2025-06-20"],  # Scaled date range
    "time_period": "day"
}
```

### Error Handling

```python
from nse_client import HistoricalDataError, AuthenticationError

try:
    data = client.get_historical_data("NIFTY", limit=100)
except AuthenticationError:
    print("❌ Authentication required for historical data")
except HistoricalDataError as e:
    print(f"❌ Historical data error: {e}")
except Exception as e:
    print(f"❌ Unexpected error: {e}")
```

## 📡 Admin Broadcast Controls

Admin users can control the real-time data broadcasting system:

### Broadcast Status

```python
# Get current broadcast status (admin only)
try:
    status = client.get_broadcast_status()
    if status:
        print(f"State: {status['state']}")
        print(f"Symbols: {status['symbol_count']}")
        print(f"Records: {status['total_records']}")
except AdminError as e:
    print(f"Admin access required: {e}")
```

### Broadcast Control Operations

```python
from nse_client import AdminError, AuthenticationError

try:
    # Start broadcasting
    if client.start_broadcast():
        print("✅ Broadcasting started")
    
    # Pause broadcasting
    if client.pause_broadcast():
        print("⏸️ Broadcasting paused")
    
    # Resume broadcasting
    if client.resume_broadcast():
        print("▶️ Broadcasting resumed")
    
    # Restart from beginning
    if client.restart_broadcast():
        print("🔄 Broadcasting restarted")
    
    # Stop broadcasting
    if client.stop_broadcast():
        print("⏹️ Broadcasting stopped")
        
except AdminError as e:
    print(f"❌ Admin operation failed: {e}")
except AuthenticationError:
    print("❌ Admin authentication required")
```

### Admin Authentication

```python
# Authenticate as admin
client = NSEClient("ws://localhost:8080", "http://localhost:3000")
if client.authenticate("admin_user"):
    print("✅ Admin authenticated")
    
    # Now admin operations will work
    status = client.get_broadcast_status()
    client.start_broadcast()
else:
    print("❌ Admin authentication failed")
```

## 📦 Order Management

### Placing Orders

```python
# Market Order
order = client.place_order(
    symbol="NIFTY",
    side="buy",           # "buy" or "sell"
    order_type="market",  # "market", "limit", "stop_loss"
    quantity=100
)

# Limit Order
order = client.place_order(
    symbol="INDIGO",
    side="sell",
    order_type="limit",
    quantity=50,
    price=1250.75        # Required for limit orders
)

# Stop Loss Order
order = client.place_order(
    symbol="NIFTY",
    side="sell",
    order_type="stop_loss",
    quantity=200,
    stop_price=21000.0   # Required for stop-loss orders
)

if order:
    print(f"✅ Order placed: {order['id']}")
    print(f"Status: {order['status']}")
```

### Order Management

```python
# Get all orders
orders = client.get_orders()
print(f"Total orders: {len(orders)}")

# Get orders by symbol
nifty_orders = client.get_orders(symbol="NIFTY")

# Get orders by status
pending_orders = client.get_orders(status="pending")

# Get specific order
order_details = client.get_order(order_id)

# Cancel order
success = client.cancel_order(order_id)
if success:
    print("✅ Order cancelled")
```

### Order Status Callbacks

```python
def on_order_update(order):
    """Called when order status changes"""
    status = order["status"]
    symbol = order["symbol"]
    side = order["side"]
    quantity = order["quantity"]
    
    print(f"📦 {status.upper()}: {side.upper()} {quantity} {symbol}")

client.on_order_update = on_order_update
```

## 🔧 Complete Example (Breeze-like Pattern)

```python
#!/usr/bin/env python3
from nse_client import NSEClient
import time

# Initialize client (similar to breeze.generate_session)
client = NSEClient(
    ws_uri="ws://localhost:8080",
    api_uri="http://localhost:3000",
    token="your-jwt-token"
)

# Connect to WebSocket (similar to breeze.ws_connect)
client.ws_connect()

# Callback to receive ticks (similar to breeze pattern)
def on_ticks(ticks):
    print(f"Ticks: {ticks}")

def on_order_update(order):
    print(f"Order Update: {order['status']} - {order['symbol']}")

# Assign callbacks (similar to breeze pattern)
client.on_ticks = on_ticks
client.on_order_update = on_order_update

# Subscribe to data feed
client.subscribe_feed("NIFTY")

# Place orders
order = client.place_order("NIFTY", "buy", "market", 100)
if order:
    order_id = order["id"]
    
    # Cancel order after some time
    time.sleep(5)
    client.cancel_order(order_id)

# Let data stream for a while
time.sleep(10)

# Disconnect (similar to breeze.ws_disconnect)
client.ws_disconnect()
```

## 🎛️ Connection Management

### Connection Status

```python
# Check if connected
if client.is_connected():
    print("✅ WebSocket is connected")

# Check API health
if client.health_check():
    print("✅ API server is healthy")
```

### Auto-Reconnection

The client automatically handles reconnections:

```python
# Configure reconnection (optional)
client.auto_reconnect = True
client.reconnect_interval = 5  # seconds
client.max_reconnect_attempts = 10
```

### Graceful Shutdown

```python
import signal
import sys

def signal_handler(sig, frame):
    print("Shutting down...")
    client.ws_disconnect()
    sys.exit(0)

signal.signal(signal.SIGINT, signal_handler)
```

## 📊 Advanced Features

### Logging

```python
# Set log level
client.set_log_level("DEBUG")  # DEBUG, INFO, WARNING, ERROR, CRITICAL
```

### Error Handling

```python
def on_error(error):
    """Handle WebSocket errors"""
    print(f"WebSocket Error: {error}")

client.on_error = on_error
```

### Multiple Clients

```python
# You can create multiple clients for different purposes
data_client = NSEClient(ws_uri, api_uri, token)
order_client = NSEClient(ws_uri, api_uri, token)

# Use data_client for streaming
data_client.on_ticks = handle_data
data_client.ws_connect()
data_client.subscribe_feed("NIFTY")

# Use order_client for trading
orders = order_client.get_orders()
```

## 🔑 Authentication

### Token Management

```python
# Method 1: Auto-load from file
client = create_client()  # Loads from test_tokens.json

# Method 2: Explicit token
client = NSEClient(ws_uri, api_uri, "your-jwt-token")

# Method 3: Environment variable
import os
token = os.getenv("NSE_JWT_TOKEN")
client = NSEClient(ws_uri, api_uri, token)
```

## 📋 API Reference

### NSEClient Methods

#### Connection Management
- `ws_connect()` → `bool` - Connect to WebSocket
- `ws_disconnect()` → `None` - Disconnect from WebSocket
- `is_connected()` → `bool` - Check connection status
- `health_check()` → `bool` - Check API health

#### Data Streaming
- `subscribe_feed(symbol)` → `bool` - Subscribe to symbol
- `unsubscribe_feed(symbol)` → `bool` - Unsubscribe from symbol
- `get_current_symbol()` → `str` - Get current subscription

#### Order Management
- `place_order(symbol, side, order_type, quantity, price=None, stop_price=None)` → `dict`
- `cancel_order(order_id)` → `bool`
- `get_orders(symbol=None, status=None)` → `list`
- `get_order(order_id)` → `dict`

#### Callbacks
- `on_ticks` - Real-time data callback
- `on_connect` - Connection established callback
- `on_disconnect` - Connection closed callback
- `on_error` - Error occurred callback
- `on_order_update` - Order status changed callback

### Order Types

| Type | Required Fields | Optional Fields |
|------|----------------|----------------|
| `market` | symbol, side, quantity | - |
| `limit` | symbol, side, quantity, price | - |
| `stop_loss` | symbol, side, quantity, stop_price | - |

### Order Statuses

- `pending` - Order placed, waiting for execution
- `filled` - Order executed successfully  
- `cancelled` - Order cancelled by user
- `rejected` - Order rejected due to validation

## 🧪 Testing

Run the example scripts:

```bash
# Full example with all features
python3 example_usage.py

# Quick tests
python3 nse_client.py  # Built-in test
```

## 🔧 Configuration

### Default Settings

```python
# WebSocket settings
ws_uri = "ws://localhost:8080"
api_uri = "http://localhost:3000" 

# Reconnection settings
auto_reconnect = True
reconnect_interval = 5  # seconds
max_reconnect_attempts = 10

# Timeouts
connection_timeout = 10  # seconds
api_timeout = 10  # seconds
```

## 🚨 Error Handling

Common error scenarios and handling:

```python
# Connection errors
if not client.ws_connect():
    print("Failed to connect - check server status")

# Order errors
order = client.place_order("INVALID", "buy", "market", 100)
if not order:
    print("Order failed - check symbol and parameters")

# API errors
if not client.health_check():
    print("API server not responding")
```

## 📝 License

MIT License - Use freely in your projects.

## 🤝 Support

For issues and questions:
1. Check the server is running: `http://localhost:3000/api/health`
2. Verify WebSocket connection: `ws://localhost:8080`
3. Check JWT token validity
4. Review server logs for authentication issues

Happy Trading! 📈 