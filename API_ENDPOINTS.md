# NSE Socket API Endpoints

## Authentication Endpoints

### POST /api/login
Generate a JWT token for a given username.

**Request:**
```json
{
    "username": "admin"
}
```

**Response (Success):**
```json
{
    "success": true,
    "message": "Token generated successfully",
    "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "user_id": "admin",
    "permissions": ["admin", "user"]
}
```

**Response (Error):**
```json
{
    "success": false,
    "message": "Username cannot be empty",
    "token": null,
    "user_id": null,
    "permissions": null
}
```

**User Permissions:**
- `admin` username gets: `["admin", "user"]` permissions
- Any other username gets: `["user"]` permissions

## Admin Broadcasting Control Endpoints

⚠️ **Important:** Broadcasting does NOT start automatically. It must be started manually by an admin using these endpoints.

### POST /api/start-broadcast
Start data broadcasting from CSV files (requires admin token).

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**What it does:**
1. Loads stock data from the `./data/` directory (all `.csv` files)
2. Creates separate broadcasting channels for each symbol (filename becomes symbol name)
3. Starts broadcasting data at 1-second intervals to subscribed WebSocket clients
4. Falls back to single file mode (`./data/NIFTY.csv`) if directory loading fails

**Response (Success):**
```json
{
    "success": true,
    "message": "Broadcasting started for 5 symbols with 2500 total records"
}
```

**Response (Error):**
```json
{
    "success": false,
    "message": "Cannot execute Start while in state Running"
}
```

### POST /api/pause-broadcast
Pause active data broadcasting (requires admin token).

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response (Success):**
```json
{
    "success": true,
    "message": "Broadcasting paused successfully"
}
```

**Response (Error):**
```json
{
    "success": false,
    "message": "Cannot execute Pause while in state Stopped"
}
```

### POST /api/resume-broadcast
Resume paused data broadcasting (requires admin token).

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response (Success):**
```json
{
    "success": true,
    "message": "Broadcasting resumed successfully"
}
```

**Response (Error):**
```json
{
    "success": false,
    "message": "Cannot execute Resume while in state Running"
}
```

### POST /api/stop-broadcast
Stop data broadcasting completely (requires admin token).

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response (Success):**
```json
{
    "success": true,
    "message": "Broadcasting stopped successfully"
}
```

### POST /api/restart-broadcast
Restart data broadcasting (stop + start) (requires admin token).

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response (Success):**
```json
{
    "success": true,
    "message": "Broadcasting started for 5 symbols with 2500 total records"
}
```

### GET /api/broadcast-status
Get current broadcasting status (requires admin token).

**Headers:**
```
Authorization: Bearer <jwt_token>
```

**Response:**
```json
{
    "success": true,
    "state": "Running",
    "symbol_count": 5,
    "total_records": 2500,
    "message": "Broadcasting is Running with 5 symbols and 2500 total records"
}
```

**Possible States:**
- `"Stopped"` - No broadcasting active
- `"Running"` - Broadcasting data normally  
- `"Paused"` - Broadcasting paused (can be resumed)

## Broadcasting State Machine

```
Stopped ──start──> Running ──pause──> Paused
   ↑                 │                   │
   │                 │                   │
   └─────stop────────┘                   │
   │                                     │
   └─────────────stop──────resume────────┘

restart = stop + start (from any state)
```

## Usage Examples

### 1. Get JWT Token
```bash
curl -X POST http://localhost:3000/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin"}'
```

### 2. Start Broadcasting
```bash
TOKEN=$(curl -s -X POST http://localhost:3000/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin"}' | jq -r '.token')

curl -X POST http://localhost:3000/api/start-broadcast \
  -H "Authorization: Bearer $TOKEN"
```

### 3. Check Broadcasting Status
```bash
curl -X GET http://localhost:3000/api/broadcast-status \
  -H "Authorization: Bearer $TOKEN"
```

### 4. Pause Broadcasting
```bash
curl -X POST http://localhost:3000/api/pause-broadcast \
  -H "Authorization: Bearer $TOKEN"
```

### 5. Resume Broadcasting
```bash
curl -X POST http://localhost:3000/api/resume-broadcast \
  -H "Authorization: Bearer $TOKEN"
```

### 6. Stop Broadcasting
```bash
curl -X POST http://localhost:3000/api/stop-broadcast \
  -H "Authorization: Bearer $TOKEN"
```

### 7. Restart Broadcasting
```bash
curl -X POST http://localhost:3000/api/restart-broadcast \
  -H "Authorization: Bearer $TOKEN"
```

## Data Directory Structure

For multi-symbol broadcasting, organize your data like this:
```
data/
  ├── NIFTY.csv      # Will broadcast as "NIFTY" symbol
  ├── RELIANCE.csv   # Will broadcast as "RELIANCE" symbol  
  ├── TCS.csv        # Will broadcast as "TCS" symbol
  └── INFY.csv       # Will broadcast as "INFY" symbol
```

**CSV Format:** Each file should have columns: `date,open,high,low,close,volume`

## Broadcasting Behavior

- **Default State:** Stopped (no automatic startup)
- **Interval:** Data broadcasts every 1 second per symbol
- **Concurrent:** All symbols broadcast simultaneously 
- **WebSocket Integration:** Data goes to PubSub system for WebSocket subscribers
- **Stateful:** Maintains state across pause/resume/stop operations
- **Admin Control:** Only admin users can control broadcasting

## Server Configuration

The API server runs on port 3000 by default. You can configure this with the `API_BIND_ADDRESS` environment variable:

```bash
export API_BIND_ADDRESS="127.0.0.1:3000"
```

## JWT Configuration

JWT tokens expire after 24 hours. You can configure the JWT secret with the `JWT_SECRET` environment variable:

```bash
export JWT_SECRET="your-super-secret-key-at-least-32-characters-long"
```

**⚠️ Important:** Always use a strong, unique JWT secret in production!

## Available Endpoints Summary

| Method | Endpoint | Auth Required | Admin Only | Description |
|--------|----------|---------------|------------|-------------|
| POST | `/api/login` | No | No | Get JWT token for username |
| POST | `/api/start-broadcast` | Yes | Yes | Start data broadcasting |
| POST | `/api/pause-broadcast` | Yes | Yes | Pause active broadcasting |
| POST | `/api/resume-broadcast` | Yes | Yes | Resume paused broadcasting |
| POST | `/api/stop-broadcast` | Yes | Yes | Stop broadcasting completely |
| POST | `/api/restart-broadcast` | Yes | Yes | Restart broadcasting (stop + start) |
| GET | `/api/broadcast-status` | Yes | Yes | Get current broadcasting status |
| GET | `/api/health` | No | No | Health check |
| POST | `/api/orders` | Yes | No | Place trading order |
| GET | `/api/orders` | Yes | No | Get user's orders |
| GET | `/api/orders/:id` | Yes | No | Get specific order |
| DELETE | `/api/orders/:id` | Yes | No | Cancel order | 