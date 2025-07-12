# NSE Socket Server

A high-performance Rust WebSocket server for real-time stock market data streaming, order management, and trading operations. This server provides both WebSocket and HTTP API endpoints with JWT-based authentication.

## 🚀 Features

- **Real-time Data Streaming**: WebSocket-based stock market data broadcasting
- **Order Management**: Full trading order lifecycle management
- **Dual-Server Architecture**: Separate WebSocket (8080) and HTTP API (3000) servers
- **JWT Authentication**: Secure token-based authentication system
- **Admin Dashboard**: Real-time admin interface for monitoring and control
- **Multi-Symbol Support**: Concurrent data streaming for multiple stock symbols
- **Session Management**: Automatic session cleanup and connection monitoring
- **Broadcasting Control**: Admin-controlled data broadcasting with pause/resume capabilities

## 📋 Table of Contents

- [Quick Start](#quick-start)
- [Installation](#installation)
- [Configuration](#configuration)
- [Server Architecture](#server-architecture)
- [API Endpoints](#api-endpoints)
- [WebSocket Endpoints](#websocket-endpoints)
- [Data Format](#data-format)
- [Security](#security)
- [Development](#development)
- [Deployment](#deployment)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ installed
- CSV data files in `./data/` directory
- Linux/macOS/Windows environment

### Running the Server

```bash
# Clone the repository
git clone https://github.com/Siddid-Soni/rust-websocket.git
cd rust-websocket

# Build the server
cargo build --release

# Run the server
./target/release/nse_socket
```

The server will start with:
- **WebSocket Server**: `ws://0.0.0.0:8080` (paths: `/ws`, `/admin`)
- **HTTP API Server**: `http://0.0.0.0:3000`

### First Steps

1. **Get Authentication Token**:
   ```bash
   curl -X POST http://localhost:3000/api/login \
     -H "Content-Type: application/json" \
     -d '{"username": "admin"}'
   ```

2. **Start Data Broadcasting**:
   ```bash
   TOKEN="your-jwt-token-here"
   curl -X POST http://localhost:3000/api/start-broadcast \
     -H "Authorization: Bearer $TOKEN"
   ```

3. **Connect to WebSocket**:
   ```bash
   # Normal WebSocket connection
   websocat ws://localhost:8080/ws
   
   # Admin WebSocket connection
   websocat "ws://localhost:8080/admin?token=$TOKEN"
   ```

## 🛠️ Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/Siddid-Soni/rust-websocket.git
cd rust-websocket

# Build in release mode
cargo build --release

# Binary will be available at ./target/release/nse_socket
```

### Development Build

```bash
# Build for development
cargo build

# Run in development mode
cargo run
```

### Data Setup

Create a `data/` directory with CSV files:

```bash
mkdir -p data
# Add your CSV files (e.g., NIFTY.csv, RELIANCE.csv, etc.)
```

**CSV Format**: Each file should have columns: `date,open,high,low,close,volume`

Example:
```csv
date,open,high,low,close,volume
2024-01-15,21500.0,21650.0,21480.0,21620.0,1500000
2024-01-16,21620.0,21780.0,21590.0,21750.0,1600000
```

## ⚙️ Configuration

The server uses environment variables for configuration:

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BIND_ADDRESS` | `0.0.0.0:8080` | WebSocket server bind address |
| `API_BIND_ADDRESS` | `0.0.0.0:3000` | HTTP API server bind address |
| `JWT_SECRET` | Auto-generated | JWT signing secret (min 32 chars) |
| `RUST_LOG` | `info` | Log level (debug, info, warn, error) |
| `DATA_FILE` | `./data/NIFTY.csv` | Primary data file path |

### Configuration Example

```bash
# Set environment variables
export BIND_ADDRESS="127.0.0.1:8080"
export API_BIND_ADDRESS="127.0.0.1:3000"
export JWT_SECRET="your-super-secret-key-at-least-32-characters-long"
export RUST_LOG="debug"

# Run the server
./target/release/nse_socket
```

### Production Configuration

For production deployment, create a `.env` file:

```env
BIND_ADDRESS=0.0.0.0:8080
API_BIND_ADDRESS=0.0.0.0:3000
JWT_SECRET=your-production-secret-key-minimum-32-characters
RUST_LOG=info
DATA_FILE=./data/NIFTY.csv
```

## 🏗️ Server Architecture

### Components Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    NSE Socket Server                        │
├─────────────────────────┬───────────────────────────────────┤
│    WebSocket Server     │         HTTP API Server          │
│      (Port 8080)        │         (Port 3000)              │
├─────────────────────────┼───────────────────────────────────┤
│  • /ws (Normal clients) │  • /api/login (Authentication)   │
│  • /admin (Admin feed)  │  • /api/orders (Order management)│
│                         │  • /api/start-broadcast (Admin)  │
├─────────────────────────┴───────────────────────────────────┤
│                     Core Services                           │
│  • Session Manager    • PubSub System   • Order Manager   │
│  • Data Broadcaster   • JWT Generator   • Config Manager  │
└─────────────────────────────────────────────────────────────┘
```

### Core Services

#### 1. Session Manager
- JWT token validation and management
- Session cleanup and monitoring
- Connection tracking and heartbeat

#### 2. PubSub System
- Real-time message broadcasting
- Symbol-based subscription management
- Multi-client data distribution

#### 3. Order Manager
- Trading order lifecycle management
- Order validation and execution
- Real-time order status updates

#### 4. Data Broadcaster
- CSV data loading and parsing
- Timed data broadcasting (1-second intervals)
- Multi-symbol concurrent streaming

#### 5. Broadcasting Controller
- Admin-controlled broadcast state machine
- Start/Pause/Resume/Stop operations
- Real-time status monitoring

## 📡 API Endpoints

### Authentication

#### POST /api/login
Generate JWT token for authentication.

**Request**:
```json
{
    "username": "admin"
}
```

**Response**:
```json
{
    "success": true,
    "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "user_id": "admin",
    "permissions": ["admin", "user"]
}
```

### Broadcasting Control (Admin Only)

#### POST /api/start-broadcast
Start data broadcasting from CSV files.

**Headers**: `Authorization: Bearer <token>`

**Response**:
```json
{
    "success": true,
    "message": "Broadcasting started for 5 symbols with 2500 total records"
}
```

#### POST /api/pause-broadcast
Pause active broadcasting.

#### POST /api/resume-broadcast
Resume paused broadcasting.

#### POST /api/stop-broadcast
Stop broadcasting completely.

#### GET /api/broadcast-status
Get current broadcasting status.

**Response**:
```json
{
    "success": true,
    "state": "Running",
    "symbol_count": 5,
    "total_records": 2500,
    "message": "Broadcasting is Running with 5 symbols and 2500 total records"
}
```

### Order Management

#### POST /api/orders
Place a new trading order.

**Request**:
```json
{
    "symbol": "NIFTY",
    "side": "buy",
    "order_type": "market",
    "quantity": 100,
    "price": 21500.0
}
```

#### GET /api/orders
Get user's orders.

#### GET /api/orders/:id
Get specific order details.

#### DELETE /api/orders/:id
Cancel an order.

### Health Check

#### GET /api/health
Server health check endpoint.

## 🔌 WebSocket Endpoints

### Normal Client WebSocket: `/ws`

**Connection**: `ws://localhost:8080/ws`

**Authentication**: JWT token required via query parameter or header

**Messages**:
```json
// Subscribe to symbol
{"action": "subscribe", "symbol": "NIFTY"}

// Unsubscribe from symbol
{"action": "unsubscribe", "symbol": "NIFTY"}

// Heartbeat
{"action": "ping"}
```

### Admin WebSocket: `/admin`

**Connection**: `ws://localhost:8080/admin?token=<jwt-token>`

**Authentication**: Admin-level JWT token required

**Real-time Feeds**:
- Order updates
- System status changes
- Broadcasting state changes

## 📊 Data Format

### Stock Data Message

```json
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
    "timestamp": "2024-01-15T10:30:00Z"
}
```

### Order Update Message

```json
{
    "order_id": "uuid-string",
    "symbol": "NIFTY",
    "side": "buy",
    "order_type": "market",
    "quantity": 100,
    "status": "filled",
    "created_at": "2024-01-15T10:30:00Z",
    "updated_at": "2024-01-15T10:30:05Z"
}
```

## 🔒 Security

### JWT Configuration

**Important**: Always use a strong JWT secret in production:

```bash
# Generate a secure secret
openssl rand -hex 32

# Set as environment variable
export JWT_SECRET="your-generated-secret-key"
```

### Token Permissions

- **Admin users**: Full access to all endpoints and admin WebSocket
- **Regular users**: Limited to normal WebSocket and basic API endpoints
- **Token expiry**: 24 hours (configurable)

### Network Security

- **WebSocket**: Consider using WSS (WebSocket Secure) in production
- **HTTP API**: Use HTTPS with proper certificates
- **CORS**: Configure CORS policies for web clients
- **Rate limiting**: Implement rate limiting for API endpoints

### Best Practices

1. **Never expose JWT secrets** in code or logs
2. **Use HTTPS/WSS** in production environments
3. **Implement proper CORS** policies
4. **Monitor failed authentication** attempts
5. **Regularly rotate JWT secrets**
6. **Use reverse proxy** (nginx/Apache) for SSL termination

## 🛠️ Development

### Prerequisites

- Rust 1.70+
- Git
- Basic knowledge of WebSocket and HTTP protocols

### Development Setup

```bash
# Clone the repository
git clone https://github.com/Siddid-Soni/rust-websocket.git
cd rust-websocket

# Install dependencies
cargo build

# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

### Project Structure

```
src/
├── main.rs                 # Main server entry point
├── config.rs              # Configuration management
├── api/                   # HTTP API handlers
│   ├── mod.rs
│   └── handlers.rs
├── websocket/             # WebSocket handlers
│   ├── mod.rs
│   ├── handler.rs
│   └── admin.rs
├── auth/                  # Authentication system
│   ├── mod.rs
│   ├── jwt.rs
│   └── session.rs
├── data/                  # Data management
│   ├── mod.rs
│   ├── loader.rs
│   ├── pubsub.rs
│   └── controller.rs
└── trading/               # Order management
    ├── mod.rs
    └── order.rs
```

### Adding New Features

1. **New API Endpoint**:
   - Add handler in `src/api/handlers.rs`
   - Update router in `src/api/mod.rs`

2. **New WebSocket Message**:
   - Update handlers in `src/websocket/handler.rs`
   - Add message types as needed

3. **New Configuration**:
   - Add to `src/config.rs`
   - Update environment variable documentation

### Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Test with specific log level
RUST_LOG=debug cargo test
```

## 🚀 Deployment

### Production Deployment

#### 1. Build for Production

```bash
# Build optimized binary
cargo build --release

# Binary will be at ./target/release/nse_socket
```

#### 2. Environment Setup

Create production configuration:

```bash
# /etc/nse-socket/config.env
BIND_ADDRESS=0.0.0.0:8080
API_BIND_ADDRESS=0.0.0.0:3000
JWT_SECRET=your-production-secret-key-minimum-32-characters
RUST_LOG=info
DATA_FILE=/var/lib/nse-socket/data/NIFTY.csv
```

#### 3. Systemd Service

Create `/etc/systemd/system/nse-socket.service`:

```ini
[Unit]
Description=NSE Socket Server
After=network.target

[Service]
Type=simple
User=nse-socket
Group=nse-socket
WorkingDirectory=/opt/nse-socket
ExecStart=/opt/nse-socket/nse_socket
EnvironmentFile=/etc/nse-socket/config.env
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

#### 4. Start Service

```bash
# Enable and start service
sudo systemctl enable nse-socket
sudo systemctl start nse-socket

# Check status
sudo systemctl status nse-socket
```

### Docker Deployment

#### Dockerfile

```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/nse_socket /usr/local/bin/nse_socket
COPY --from=builder /app/data /app/data

WORKDIR /app

EXPOSE 8080 3000

CMD ["nse_socket"]
```

#### Docker Compose

```yaml
version: '3.8'

services:
  nse-socket:
    build: .
    ports:
      - "8080:8080"
      - "3000:3000"
    environment:
      - BIND_ADDRESS=0.0.0.0:8080
      - API_BIND_ADDRESS=0.0.0.0:3000
      - JWT_SECRET=your-production-secret-key-minimum-32-characters
      - RUST_LOG=info
    volumes:
      - ./data:/app/data
    restart: unless-stopped
```

### Reverse Proxy Setup

#### Nginx Configuration

```nginx
upstream nse_api {
    server 127.0.0.1:3000;
}

upstream nse_ws {
    server 127.0.0.1:8080;
}

server {
    listen 80;
    server_name your-domain.com;

    location /api/ {
        proxy_pass http://nse_api;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /ws {
        proxy_pass http://nse_ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /admin {
        proxy_pass http://nse_ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

## 📊 Monitoring

### Logging

The server provides comprehensive logging:

```bash
# View logs in real-time
sudo journalctl -u nse-socket -f

# View recent logs
sudo journalctl -u nse-socket -n 100

# View logs for specific date
sudo journalctl -u nse-socket --since "2024-01-15"
```

### Health Monitoring

#### Health Check Endpoint

```bash
# Basic health check
curl http://localhost:3000/api/health

# Response
{
    "status": "healthy",
    "timestamp": "2024-01-15T10:30:00Z",
    "uptime": "1h 30m 45s"
}
```

#### Key Metrics

The server logs important metrics:

- **Active sessions**: Number of connected clients
- **Symbol subscriptions**: Data feed subscriptions
- **Order statistics**: Trading order counts
- **Broadcasting status**: Data streaming state

### Performance Monitoring

#### Log Analysis

```bash
# Monitor connection patterns
grep "WebSocket connection" /var/log/nse-socket.log

# Monitor authentication failures
grep "authentication failed" /var/log/nse-socket.log

# Monitor order activity
grep "Order" /var/log/nse-socket.log
```

#### Resource Usage

```bash
# Monitor CPU and memory usage
top -p $(pgrep nse_socket)

# Monitor network connections
netstat -tuln | grep -E "8080|3000"
```

## 🔧 Troubleshooting

### Common Issues

#### 1. Server Won't Start

**Problem**: Server exits immediately after starting

**Solutions**:
- Check data file exists: `ls -la data/NIFTY.csv`
- Verify JWT secret length: `echo $JWT_SECRET | wc -c` (should be ≥32)
- Check port availability: `netstat -tuln | grep -E "8080|3000"`

#### 2. Authentication Failures

**Problem**: JWT token validation fails

**Solutions**:
- Verify JWT secret consistency
- Check token expiry (24 hours)
- Ensure proper Authorization header format

#### 3. WebSocket Connection Issues

**Problem**: WebSocket connections fail or disconnect

**Solutions**:
- Check WebSocket path (`/ws` or `/admin`)
- Verify JWT token in query parameter
- Monitor network connectivity
- Check firewall rules

#### 4. Data Broadcasting Not Working

**Problem**: No data being broadcast to clients

**Solutions**:
- Start broadcasting: `curl -X POST http://localhost:3000/api/start-broadcast -H "Authorization: Bearer $TOKEN"`
- Check data files exist in `./data/` directory
- Verify CSV format and content
- Check admin permissions

### Debug Mode

Run server with debug logging:

```bash
RUST_LOG=debug ./target/release/nse_socket
```

### Log Levels

- **DEBUG**: Detailed debugging information
- **INFO**: General operational information
- **WARN**: Warning conditions
- **ERROR**: Error conditions

### Getting Help

1. **Check server logs**: `sudo journalctl -u nse-socket -f`
2. **Verify configuration**: Review environment variables
3. **Test endpoints**: Use curl to test API endpoints
4. **Check network**: Verify ports are accessible
5. **Review documentation**: Check API_ENDPOINTS.md for API details

## 📚 Related Documentation

- [API Endpoints Documentation](API_ENDPOINTS.md) - Complete API reference
- [Python Client Library](NSE_CLIENT_README.md) - Client-side usage guide
- [Rust WebSocket Documentation](https://docs.rs/tokio-tungstenite/) - WebSocket library reference

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [Tokio](https://tokio.rs/) for async runtime
- Uses [Axum](https://github.com/tokio-rs/axum) for HTTP server
- WebSocket support via [tokio-tungstenite](https://github.com/snapview/tokio-tungstenite)
- JWT authentication with [jsonwebtoken](https://github.com/Keats/jsonwebtoken)

---

**Happy Trading!** 📈🚀