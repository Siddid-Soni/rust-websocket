# NSE Socket Server with JWT Authentication

A production-ready WebSocket server for real-time stock data streaming with JWT-based authentication and session management.

## 🏗️ Architecture

The server is built with a clean, modular architecture:

```
src/
├── main.rs         # Server entry point and coordination
├── config.rs       # Configuration management
├── jwt.rs          # JWT token validation and parsing
├── session.rs      # Session management and tracking
├── data.rs         # Data loading and broadcasting
└── websocket.rs    # WebSocket connection handling
```

## Features

- ✅ **JWT Authentication**: Secure token-based authentication
- ✅ **Session Management**: One session per JWT ID (jti claim)
- ✅ **Real-time Data**: Live stock price streaming
- ✅ **Connection Limits**: Configurable maximum connections
- ✅ **Heartbeat System**: Automatic stale connection cleanup
- ✅ **Production Ready**: Proper error handling and logging
- ✅ **Concurrent Safety**: Thread-safe token management
- ✅ **Modular Design**: Clean separation of concerns
- ✅ **Comprehensive Testing**: Unit tests for all modules

## Quick Start

### 1. Build and Run the Server

```bash
# Install dependencies
cargo build --release

# Set environment variables (optional)
export JWT_SECRET="your-super-secret-key-here"
export RUST_LOG="info"
export BIND_ADDRESS="127.0.0.1:8080"
export DATA_FILE="./src/data.csv"

# Run the server
cargo run
# or
./target/release/nse_socket
```

The server will start on `ws://localhost:8080`

### 2. Generate JWT Tokens

```bash
# Install Python dependencies
pip install PyJWT

# Generate test tokens
python3 generate_jwt.py

# Verify a token
python3 generate_jwt.py verify <your-jwt-token>
```

### 3. Test with curl

```bash
# Get a token from the Python script, then:
curl -H "Authorization: Bearer YOUR_JWT_TOKEN" --websocket ws://localhost:8080
```

## 🔧 Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `JWT_SECRET` | `your-secret-key-change-in-production` | JWT signing secret (min 32 chars) |
| `RUST_LOG` | `info` | Log level (error, warn, info, debug, trace) |
| `BIND_ADDRESS` | `127.0.0.1:8080` | Server bind address |
| `DATA_FILE` | `./src/data.csv` | Path to stock data CSV file |

### Server Constants

```rust
// Session Management
pub const MAX_CONNECTIONS: usize = 1000;           // Maximum concurrent connections
pub const CONNECTION_TIMEOUT_SECS: u64 = 300;     // 5 minutes session timeout
pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;      // Heartbeat interval

// Data Broadcasting
pub const DATA_BROADCAST_INTERVAL_SECS: u64 = 10; // Data broadcast interval
pub const CLEANUP_INTERVAL_SECS: u64 = 60;        // Session cleanup interval
pub const BROADCAST_CHANNEL_SIZE: usize = 100;    // Broadcast channel buffer size
```

## JWT Token Structure

The server expects JWT tokens with the following claims:

```json
{
  "sub": "user123",                           // Subject (user ID)
  "jti": "unique-session-id",                 // JWT ID (session identifier)
  "exp": 1738632400,                         // Expiration timestamp
  "iat": 1738628800,                         // Issued at timestamp
  "user_id": "user123",                      // User identifier
  "permissions": ["read_data", "websocket_connect"]  // User permissions
}
```

## 🔐 Authentication Flow

1. **Client Request**: Client connects with `Authorization: Bearer <JWT>` header
2. **JWT Validation**: Server validates token signature, expiration, and claims
3. **Session Check**: Server checks if JWT ID (`jti`) is already active
4. **Connection Accept/Reject**: 
   - ✅ Valid + Available → Accept connection
   - ❌ Invalid/Expired → 401 Unauthorized
   - ❌ Already Active → 409 Conflict
   - ❌ Max Connections → 503 Service Unavailable

## 📊 Session Management

### One Session per JWT ID

Each JWT's `jti` (JWT ID) claim represents a unique session. Only one WebSocket connection per `jti` is allowed at any time.

```bash
# First connection - SUCCESS
curl -H "Authorization: Bearer token1" --websocket ws://localhost:8080

# Second connection with same token - CONFLICT (409)
curl -H "Authorization: Bearer token1" --websocket ws://localhost:8080
```

### Session Tracking

The server tracks:
- Session ID (`jti` from JWT)
- User ID (`user_id` from JWT)
- Connection timestamp
- Last heartbeat
- User permissions

### Automatic Cleanup

- **Heartbeat**: Every 30 seconds to track active connections
- **Timeout**: 5-minute inactivity timeout
- **Cleanup**: Stale sessions removed every minute

## 🚀 Production Deployment

### 1. Security

```bash
# Generate a strong JWT secret (32+ characters)
export JWT_SECRET=$(openssl rand -base64 32)

# Use proper logging for production
export RUST_LOG="warn"

# Bind to all interfaces if needed
export BIND_ADDRESS="0.0.0.0:8080"
```

### 2. Docker Deployment

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/nse_socket /usr/local/bin/
COPY --from=builder /app/src/data.csv /app/
EXPOSE 8080

ENV DATA_FILE=/app/data.csv
ENV RUST_LOG=info

CMD ["nse_socket"]
```

### 3. Database Integration

For production, extend the modules:
- **`session.rs`**: Replace in-memory storage with Redis/PostgreSQL
- **`jwt.rs`**: Add database-backed token validation
- **`data.rs`**: Add real-time data feed integration

## 🧪 Testing

### Run Unit Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run tests for specific module
cargo test session::tests
cargo test jwt::tests
cargo test data::tests
```

### Testing Scenarios

1. **Valid Connection**: Use generated JWT tokens
2. **Concurrent Sessions**: Different JWT IDs should work
3. **Session Conflict**: Same JWT ID should be rejected
4. **Token Expiration**: Expired tokens should be rejected
5. **Invalid Tokens**: Malformed JWTs should be rejected

## 📁 Module Documentation

### `config.rs` - Configuration Management
- Environment variable handling
- Configuration validation
- Default value management

### `jwt.rs` - JWT Authentication
- Token validation and parsing
- Claims extraction
- Security verification

### `session.rs` - Session Management
- Active session tracking
- Heartbeat monitoring
- Stale session cleanup

### `data.rs` - Data Handling
- CSV data loading
- Stock data broadcasting
- Error resilient parsing

### `websocket.rs` - Connection Management
- WebSocket handshake handling
- Message routing
- Connection lifecycle management

### `main.rs` - Server Coordination
- Module integration
- Background task management
- Server lifecycle

## 🔍 Monitoring

The server provides detailed logging for:
- Connection events
- Authentication results
- Session management
- Data broadcasting
- Error conditions

## 📈 Performance

- **Concurrent Connections**: Up to 1,000 simultaneous connections
- **Memory Usage**: Efficient Arc/Mutex usage for shared state
- **CPU Usage**: Async/await for non-blocking operations
- **Network**: Optimized WebSocket message handling

## Files

- `src/main.rs` - Server entry point and coordination
- `src/config.rs` - Configuration management
- `src/jwt.rs` - JWT token validation
- `src/session.rs` - Session management
- `src/data.rs` - Data loading and broadcasting
- `src/websocket.rs` - WebSocket connection handling
- `generate_jwt.py` - JWT token generator
- `src/data.csv` - Sample stock data
- `Cargo.toml` - Rust dependencies

## License

MIT License - See LICENSE file for details. 