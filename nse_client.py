#!/usr/bin/env python3
"""
NSE Socket Client Library
A simple Python client library for the NSE Socket server with WebSocket and REST API support.
"""

import json
import time
import threading
import requests
import websocket
from typing import Optional, Dict, Any, Callable, List, Set, Union
from datetime import datetime, date
import logging
import signal
import sys

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)
logger = logging.getLogger(__name__)


def get_token(api_uri: str, username: str) -> Optional[str]:
    """
    Standalone function to request JWT token from the NSE server.
    
    Args:
        api_uri: HTTP API server URI (e.g., "http://localhost:3000")
        username: Username for token generation
        
    Returns:
        str: JWT token string if successful, None if failed
        
    Raises:
        AuthenticationError: If token retrieval fails
    """
    try:
        # Fix URL formatting - if api_uri is just hostname, add http:// and port
        if "://" not in api_uri:
            login_url = f"http://{api_uri}:3000/api/login"
        else:
            login_url = f"{api_uri}/api/login"
        
        response = requests.post(
            login_url,
            json={"username": username},
            headers={'Content-Type': 'application/json'},
            timeout=10
        )
        
        if response.status_code == 200:
            data = response.json()
            if data.get("success") and data.get("token"):
                token = data["token"]
                return token
            else:
                error_msg = data.get("message", "Token generation failed")
                raise AuthenticationError(f"Authentication failed: {error_msg}")
        else:
            raise AuthenticationError(f"Authentication failed: HTTP {response.status_code}")
            
    except requests.exceptions.RequestException as e:
        raise AuthenticationError(f"Network error: {str(e)}")
    except Exception as e:
        raise AuthenticationError(f"Authentication error: {str(e)}")


class NSESocketError(Exception):
    """Base exception for NSE Socket client errors."""
    pass


class AuthenticationError(NSESocketError):
    """Exception raised for authentication-related errors."""
    pass


class ConnectionError(NSESocketError):
    """Exception raised for connection-related errors."""
    pass


class SubscriptionError(NSESocketError):
    """Exception raised for subscription-related errors."""
    pass


class HistoricalDataError(NSESocketError):
    """Exception raised for historical data API errors."""
    pass


class NSEClient:
    """
    NSE Socket Client for real-time market data streaming and historical data access.
    
    This client provides an interface for connecting to the NSE Socket server,
    subscribing to real-time market data feeds, and accessing historical market data.
    
    Example Usage:
        # Basic usage
        client = NSEClient("localhost")
        if client.authenticate("admin"):
            client.connect_and_subscribe(["NIFTY", "RELIANCE", "TCS"])
            client.run()
    """
    
    def __init__(self, uri, token: Optional[str] = None):
        """
        Initialize NSE Socket Client.
        
        Args:
            uri: Server hostname or IP (e.g., "localhost" or "192.168.1.100")
            token: JWT authentication token (optional, can be obtained via authenticate())
        
        Raises:
            ValueError: If URI is invalid
        """
        # Validate and normalize URIs
        if not uri:
            raise ValueError("Server URI must be provided")
            
        # Clean up the URI - remove any protocol and port if present
        clean_uri = uri.replace("ws://", "").replace("http://", "").split(":")[0]
            
        self.ws_uri = f"ws://{clean_uri}:8080/ws"
        self.api_uri = f"http://{clean_uri}:3000"
        self.token = token
        
        # Connection state
        self.ws = None
        self.connected = False
        self.running = False
        self.ws_thread = None
        self._stop_event = threading.Event()
        
        # Subscription management
        self.subscribed_symbols: Set[str] = set()
        
        # API configuration
        self.headers = self._get_headers()
        
        # Event callbacks
        self.on_ticks: Optional[Callable[[Dict], None]] = None
        self.on_connect: Optional[Callable[[], None]] = None
        self.on_disconnect: Optional[Callable[[], None]] = None
        self.on_error: Optional[Callable[[Exception], None]] = None
        
        # Connection settings
        self.auto_reconnect = True
        self.reconnect_interval = 5
        self.max_reconnect_attempts = 10
        self.reconnect_attempts = 0
        
        # Heartbeat configuration
        self.heartbeat_enabled = True
        self.heartbeat_interval = 25
        self.ping_timeout = 10
        self.last_pong_time = None
        
        # Setup graceful shutdown
        self._setup_signal_handlers()

    def _get_headers(self) -> Dict[str, str]:
        """Get HTTP headers for API requests."""
        headers = {'Content-Type': 'application/json'}
        if self.token:
            headers['Authorization'] = f'Bearer {self.token}'
        return headers

    def _setup_signal_handlers(self):
        """Configure signal handlers for graceful shutdown."""
        def signal_handler(signum, frame):
            logger.info("Shutdown signal received")
            self.force_shutdown()
            sys.exit(0)
            
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)

    def health_check(self) -> bool:
        """
        Check API server health.
        
        Returns:
            bool: True if healthy, False otherwise
        """
        try:
            response = requests.get(f"{self.api_uri}/api/health", timeout=5)
            return response.status_code == 200
        except:
            return False

    def ws_connect(self, blocking: bool = True, keep_alive: bool = True) -> bool:
        """
        Connect to WebSocket server.
        Use client.on_ticks to set callback for data streaming.
        
        Args:
            blocking: If True, waits for connection to establish. If False, returns immediately.
            keep_alive: If True (and blocking=True), keeps program alive to receive data.
        
        Returns:
            bool: True if connection successful (blocking) or initiated (non-blocking)
        """
        try:
            if self.connected:
                logger.warning("Already connected to WebSocket")
                return True
                
            # Reset connection state
            self.connected = False
            self._stop_event.clear()
            
            if blocking:
                # Synchronous connection - wait for connection before returning
                
                # Use a connection event to wait for connection
                connection_event = threading.Event()
                connection_success = [False]  # Use list to modify from callback
                
                def on_open_blocking(ws):
                    self.connected = True
                    connection_success[0] = True
                    connection_event.set()
                    if self.on_connect:
                        try:
                            self.on_connect()
                        except Exception as e:
                            logger.error(f"Connect callback error: {e}")
                
                def on_error_blocking(ws, error):
                    logger.error(f"WebSocket connection failed: {error}")
                    connection_success[0] = False
                    connection_event.set()
                    if self.on_error:
                        try:
                            self.on_error(error)
                        except Exception as e:
                            logger.error(f"Error callback failed: {e}")
                
                # Create WebSocket with blocking callbacks
                self.ws = websocket.WebSocketApp(
                    self.ws_uri,
                    header=[f"Authorization: Bearer {self.token}"],
                    on_open=on_open_blocking,
                    on_message=self._on_ws_message,
                    on_error=on_error_blocking,
                    on_close=self._on_ws_close,
                    on_ping=self._on_ws_ping,
                    on_pong=self._on_ws_pong
                )
                
                # Start WebSocket in separate thread
                self.running = True
                if self.heartbeat_enabled:
                    self.ws_thread = threading.Thread(target=self._run_websocket_with_ping)
                else:
                    self.ws_thread = threading.Thread(target=self._run_websocket_simple)
                    
                self.ws_thread.daemon = True
                self.ws_thread.start()
                
                # Wait for connection event with timeout
                if connection_event.wait(timeout=10):
                    if connection_success[0]:
                        self.reconnect_attempts = 0
                        
                        # Keep alive if requested
                        if keep_alive:
                            try:
                                # Run until stop event is set
                                self._stop_event.wait()
                            except KeyboardInterrupt:
                                logger.info("Interrupted by user")
                                self.stop()
                            except Exception as e:
                                logger.error(f"Keep alive error: {e}")
                                self.stop()
                        
                        return True
                    else:
                        logger.error("WebSocket connection failed")
                        return False
                else:
                    logger.error("WebSocket connection timeout")
                    return False
            else:
                # Non-blocking connection - original behavior
                self.ws = websocket.WebSocketApp(
                    self.ws_uri,
                    header=[f"Authorization: Bearer {self.token}"],
                    on_open=self._on_ws_open,
                    on_message=self._on_ws_message,
                    on_error=self._on_ws_error,
                    on_close=self._on_ws_close,
                    on_ping=self._on_ws_ping,
                    on_pong=self._on_ws_pong
                )
                
                # Start WebSocket in separate thread
                self.running = True
                if self.heartbeat_enabled:
                    self.ws_thread = threading.Thread(target=self._run_websocket_with_ping)
                else:
                    self.ws_thread = threading.Thread(target=self._run_websocket_simple)
                    
                self.ws_thread.daemon = True
                self.ws_thread.start()
                
                return True
                
        except Exception as e:
            logger.error(f"WebSocket connection failed: {e}")
            return False

    def run(self, timeout: Optional[float] = None):
        """
        Run the client in blocking mode until stopped or disconnected.
        This eliminates the need for sleep in your main program.
        
        Args:
            timeout: Maximum time to run in seconds (None = run indefinitely)
        """
        if not self.connected:
            logger.error("❌ Not connected. Call ws_connect() or connect_and_subscribe() first")
            return
            
        logger.info("🚀 Starting data stream... Press Ctrl+C to stop")
        
        try:
            if timeout:
                self._stop_event.wait(timeout)
            else:
                # Run until stop event is set
                self._stop_event.wait()
                
        except KeyboardInterrupt:
            logger.info("🛑 Interrupted by user - forcing shutdown")
            self.force_shutdown()
            return
        finally:
            # Only call normal stop if we weren't force-shutdown
            if self.running:
                self.stop()

    def stop(self):
        """Stop the client and disconnect."""
        # Set stop event first to signal all threads
        self._stop_event.set()
        self.running = False
        self.auto_reconnect = False
        
        # Disconnect WebSocket
        self.ws_disconnect()
        
        # Give a moment for everything to cleanup
        time.sleep(0.1)

    def ws_disconnect(self):
        """Disconnect from WebSocket server."""
        try:
            self.running = False
            self.auto_reconnect = False
            
            # Close WebSocket connection first
            if self.ws:
                try:
                    self.ws.close()
                except Exception as e:
                    logger.debug(f"Error closing WebSocket: {e}")
                    
            # Force thread to stop by setting stop event
            self._stop_event.set()
            
            # Wait for thread to finish with shorter timeout
            if self.ws_thread and self.ws_thread.is_alive():
                self.ws_thread.join(timeout=3)
                
                # If thread is still alive, it's probably stuck
                if self.ws_thread and self.ws_thread.is_alive():
                    logger.warning("WebSocket thread didn't stop cleanly")
                
            self.connected = False
            self.subscribed_symbols.clear()
            self.ws = None
            self.ws_thread = None
            
        except Exception as e:
            logger.error(f"Disconnect error: {e}")
            # Force cleanup even if there was an error
            self.connected = False
            self.running = False
            self.ws = None
            self.ws_thread = None

    def subscribe_feed(self, symbol: str) -> bool:
        """
        Subscribe to real-time data feed for a symbol.
        Can be called before connection - subscription will be queued.
        
        Args:
            symbol: Stock symbol (e.g., "NIFTY", "INDIGO")
            
        Returns:
            bool: True if subscription successful or queued, False otherwise
        """
        try:
            symbol = symbol.upper()
            
            if not self.connected:
                # Queue subscription for when connection is established
                self.subscribed_symbols.add(symbol)
                
                # Set up a callback to subscribe when connected
                original_on_connect = self.on_connect
                
                def on_connect_with_subscribe():
                    # Call original callback first
                    if original_on_connect:
                        try:
                            original_on_connect()
                        except Exception as e:
                            logger.error(f"Connect callback error: {e}")
                    
                    # Then subscribe to queued symbols
                    if self.subscribed_symbols:
                        queued_symbols = list(self.subscribed_symbols)
                        self.subscribed_symbols.clear()  # Clear queue before subscribing
                        for sym in queued_symbols:
                            self._subscribe_now_single(sym)
                
                self.on_connect = on_connect_with_subscribe
                return True  # Return success for queuing
            
            # If connected, subscribe immediately
            return self._subscribe_now_single(symbol)
            
        except Exception as e:
            logger.error(f"Subscription failed for {symbol}: {e}")
            return False

    def _subscribe_now_single(self, symbol: str) -> bool:
        """Internal method to subscribe to a single symbol immediately (when connected)."""
        try:
            message = {
                "action": "subscribe",
                "symbol": symbol
            }
            
            self.ws.send(json.dumps(message))
            self.subscribed_symbols.add(symbol)
            return True
            
        except Exception as e:
            logger.error(f"Subscription failed for {symbol}: {e}")
            return False

    def subscribe_multiple(self, symbols: List[str]) -> Dict[str, bool]:
        """
        Subscribe to real-time data feeds for multiple symbols.
        
        Args:
            symbols: List of stock symbols (e.g., ["NIFTY", "INDIGO", "RELIANCE"])
            
        Returns:
            Dict[str, bool]: Dictionary mapping symbol to subscription success status
        """
        if not self.connected:
            logger.error("Not connected to WebSocket")
            return {symbol: False for symbol in symbols}
            
        results = {}
        successful_subscriptions = []
        
        for symbol in symbols:
            try:
                symbol = symbol.upper()
                message = {
                    "action": "subscribe",
                    "symbol": symbol
                }
                
                self.ws.send(json.dumps(message))
                self.subscribed_symbols.add(symbol)
                results[symbol] = True
                successful_subscriptions.append(symbol)
                
                # Small delay between subscriptions to avoid overwhelming the server
                time.sleep(0.1)
                
            except Exception as e:
                logger.error(f"Subscription failed for {symbol}: {e}")
                results[symbol] = False
        
        return results

    def unsubscribe_feed(self, symbol: Optional[str] = None) -> bool:
        """
        Unsubscribe from data feed.
        
        Args:
            symbol: Symbol to unsubscribe from (optional, unsubscribes from all if not provided)
            
        Returns:
            bool: True if unsubscription successful, False otherwise
        """
        if not self.connected:
            logger.error("❌ Not connected to WebSocket")
            return False
            
        try:
            if symbol:
                # Unsubscribe from specific symbol
                symbol = symbol.upper()
                if symbol not in self.subscribed_symbols:
                    logger.warning(f"⚠️ Not subscribed to {symbol}")
                    return False
                    
                message = {
                    "action": "unsubscribe", 
                    "symbol": symbol
                }
                
                self.ws.send(json.dumps(message))
                self.subscribed_symbols.discard(symbol)
                logger.info(f"📡 Unsubscribed from {symbol}")
                return True
            else:
                # Unsubscribe from all symbols
                if not self.subscribed_symbols:
                    logger.warning("⚠️ No active subscriptions")
                    return False
                
                symbols_to_unsubscribe = list(self.subscribed_symbols)
                success = True
                
                for sym in symbols_to_unsubscribe:
                    try:
                        message = {
                            "action": "unsubscribe", 
                            "symbol": sym
                        }
                        self.ws.send(json.dumps(message))
                        time.sleep(0.1)  # Small delay between unsubscriptions
                    except Exception as e:
                        logger.error(f"❌ Failed to unsubscribe from {sym}: {e}")
                        success = False
                
                self.subscribed_symbols.clear()
                logger.info(f"📡 Unsubscribed from all symbols")
                return success
                
        except Exception as e:
            logger.error(f"❌ Unsubscription failed: {e}")
            return False

    def unsubscribe_multiple(self, symbols: List[str]) -> Dict[str, bool]:
        """
        Unsubscribe from multiple symbols.
        
        Args:
            symbols: List of symbols to unsubscribe from
            
        Returns:
            Dict[str, bool]: Dictionary mapping symbol to unsubscription success status
        """
        if not self.connected:
            logger.error("❌ Not connected to WebSocket")
            return {symbol: False for symbol in symbols}
            
        results = {}
        successful_unsubscriptions = []
        
        for symbol in symbols:
            try:
                symbol = symbol.upper()
                if symbol not in self.subscribed_symbols:
                    logger.warning(f"⚠️ Not subscribed to {symbol}")
                    results[symbol] = False
                    continue
                    
                message = {
                    "action": "unsubscribe", 
                    "symbol": symbol
                }
                
                self.ws.send(json.dumps(message))
                self.subscribed_symbols.discard(symbol)
                results[symbol] = True
                successful_unsubscriptions.append(symbol)
                
                # Small delay between unsubscriptions
                time.sleep(0.1)
                
            except Exception as e:
                logger.error(f"❌ Unsubscription failed for {symbol}: {e}")
                results[symbol] = False
        
        if successful_unsubscriptions:
            logger.info(f"📡 Unsubscribed from {len(successful_unsubscriptions)} symbols: {', '.join(successful_unsubscriptions)}")
        
        return results

    # Historical Data API Methods
    def get_historical_summary(self) -> Optional[Dict[str, Any]]:
        """
        Get summary of all available symbols and their record counts.
        
        Returns:
            Dict: Summary with symbols, counts, and totals, or None if failed
            
        Raises:
            HistoricalDataError: If request fails or authentication required
        """
        try:
            response = requests.get(
                f"{self.api_uri}/api/historical",
                headers=self.headers,
                timeout=10
            )
            
            if response.status_code == 200:
                result = response.json()
                if result.get("success"):
                    return result
                else:
                    raise HistoricalDataError(f"Historical summary failed: {result.get('message', 'Unknown error')}")
            elif response.status_code == 401:
                raise AuthenticationError("Authentication required for historical data access")
            else:
                raise HistoricalDataError(f"Historical summary failed: HTTP {response.status_code}")
                
        except requests.exceptions.RequestException as e:
            raise HistoricalDataError(f"Request failed: {str(e)}")
        except Exception as e:
            if isinstance(e, (HistoricalDataError, AuthenticationError)):
                raise
            raise HistoricalDataError(f"Unexpected error: {str(e)}")

    def get_historical_data(
        self,
        symbol: str,
        limit: Optional[int] = None,
        from_date: Optional[Union[str, date]] = None,
        to_date: Optional[Union[str, date]] = None,
        time_period: Optional[str] = None,
        validate_datetime: bool = True
    ) -> List[Dict[str, Any]]:
        """
        Get historical data with clean format - essential fields including datetime.
        
        Args:
            symbol: Symbol to get data for (e.g., "NIFTY", "RELIANCE")
            limit: Maximum number of records to return
            from_date: Start date filter (YYYY-MM-DD format or date object)
            to_date: End date filter (YYYY-MM-DD format or date object)
            time_period: Time period filtering - "minutes"/"min"/"m", "hour"/"hours"/"h", "day"/"days"/"d"
            validate_datetime: Whether to validate datetime format (default: True)
        
        Returns:
            List[Dict]: Clean list with [datetime, open, high, low, close, volume] fields
        """
        try:
            # Convert date objects to strings if needed
            if isinstance(from_date, date):
                from_date = from_date.strftime("%Y-%m-%d")
            if isinstance(to_date, date):
                to_date = to_date.strftime("%Y-%m-%d")
            
            # Build query parameters
            params = {}
            if limit is not None:
                params['limit'] = limit
            if from_date:
                params['from_date'] = from_date
            if to_date:
                params['to_date'] = to_date
            if time_period:
                params['time_period'] = time_period
            
            response = requests.get(
                f"{self.api_uri}/api/historical/{symbol}",
                headers=self.headers,
                params=params,
                timeout=15
            )
            
            if response.status_code == 200:
                result = response.json()
                if result.get("success"):
                    # Extract and clean the data - keep essential fields including datetime
                    clean_data = []
                    for record in result.get("data", []):
                        clean_record = {
                            "datetime": record.get("datetime"),
                            "open": record.get("open"),
                            "high": record.get("high"), 
                            "low": record.get("low"),
                            "close": record.get("close"),
                            "volume": record.get("volume")
                        }
                        
                        # Validate datetime format if requested
                        if validate_datetime and clean_record["datetime"]:
                            try:
                                # Validate YYYY-MM-DD HH:MM:SS format
                                datetime.strptime(clean_record["datetime"], "%Y-%m-%d %H:%M:%S")
                            except ValueError:
                                logger.warning(f"Invalid datetime format: {clean_record['datetime']}")
                        
                        clean_data.append(clean_record)
                    
                    return clean_data
                else:
                    logger.error(f"Historical data failed for {symbol}: {result.get('message', 'Unknown error')}")
                    return []
            elif response.status_code == 401:
                raise AuthenticationError("Authentication required for historical data access")
            elif response.status_code == 404:
                logger.error(f"Symbol {symbol} not found")
                return []
            else:
                logger.error(f"Historical data failed for {symbol}: HTTP {response.status_code}")
                return []
                
        except requests.exceptions.RequestException as e:
            logger.error(f"Request failed: {str(e)}")
            return []
        except Exception as e:
            if isinstance(e, AuthenticationError):
                raise
            logger.error(f"Unexpected error: {str(e)}")
            return []

    def get_available_symbols(self) -> List[str]:
        """Get list of available symbols."""
        try:
            summary = self.get_historical_summary()
            return summary.get("symbols", []) if summary else []
        except:
            return []

    # Order Management Methods
    def place_order(self, symbol: str, side: str, order_type: str, quantity: int, 
                   price: Optional[float] = None, stop_price: Optional[float] = None) -> Optional[Dict]:
        """
        Place a stock order.
        
        Args:
            symbol: Stock symbol
            side: "buy" or "sell"  
            order_type: "market", "limit", or "stop_loss"
            quantity: Number of shares
            price: Price for limit orders (optional)
            stop_price: Stop price for stop-loss orders (optional)
            
        Returns:
            Dict: Order details if successful, None if failed
        """
        try:
            order_data = {
                "symbol": symbol.upper(),
                "side": side.lower(),
                "order_type": order_type.lower(),
                "quantity": quantity
            }
            
            if price is not None:
                order_data["price"] = price
            if stop_price is not None:
                order_data["stop_price"] = stop_price
                
            response = requests.post(
                f"{self.api_uri}/api/orders",
                headers=self.headers,
                json=order_data,
                timeout=10
            )
            
            if response.status_code == 200:
                result = response.json()
                if result.get("success"):
                    order = result.get("order")
                    logger.info(f"Order placed: {order['id']} - {side.upper()} {quantity} {symbol}")
                    return order
                else:
                    logger.error(f"Order failed: {result.get('message')}")
                    return None
            else:
                logger.error(f"Order request failed: HTTP {response.status_code}")
                return None
                
        except Exception as e:
            logger.error(f"Order placement error: {e}")
            return None

    def cancel_order(self, order_id: str) -> bool:
        """
        Cancel an order.
        
        Args:
            order_id: Order ID to cancel
            
        Returns:
            bool: True if successful, False otherwise
        """
        try:
            response = requests.delete(
                f"{self.api_uri}/api/orders/{order_id}",
                headers=self.headers,
                timeout=10
            )
            
            if response.status_code == 200:
                result = response.json()
                if result.get("success"):
                    logger.info(f"Order cancelled: {order_id}")
                    return True
                else:
                    logger.error(f"Cancel failed: {result.get('message')}")
                    return False
            else:
                logger.error(f"Cancel request failed: HTTP {response.status_code}")
                return False
                
        except Exception as e:
            logger.error(f"Order cancellation error: {e}")
            return False

    def get_orders(self, symbol: Optional[str] = None, status: Optional[str] = None) -> List[Dict]:
        """
        Get user's orders.
        
        Args:
            symbol: Filter by symbol (optional)
            status: Filter by status (optional)
            
        Returns:
            List[Dict]: List of orders
        """
        try:
            params = {}
            if symbol:
                params['symbol'] = symbol.upper()
            if status:
                params['status'] = status.lower()
                
            response = requests.get(
                f"{self.api_uri}/api/orders",
                headers=self.headers,
                params=params,
                timeout=10
            )
            
            if response.status_code == 200:
                result = response.json()
                if result.get("success"):
                    return result.get("orders", [])
                    
            logger.error(f"❌ Failed to get orders: {response.status_code}")
            return []
            
        except Exception as e:
            logger.error(f"❌ Error getting orders: {e}")
            return []

    def get_order(self, order_id: str) -> Optional[Dict]:
        """
        Get specific order details.
        
        Args:
            order_id: Order ID
            
        Returns:
            Dict: Order details if found, None otherwise
        """
        try:
            response = requests.get(
                f"{self.api_uri}/api/orders/{order_id}",
                headers=self.headers,
                timeout=10
            )
            
            if response.status_code == 200:
                result = response.json()
                if result.get("success"):
                    return result.get("order")
                    
            logger.error(f"❌ Failed to get order: {response.status_code}")
            return None
            
        except Exception as e:
            logger.error(f"❌ Error getting order: {e}")
            return None

    # WebSocket event handlers
    def _on_ws_open(self, ws):
        """Handle WebSocket connection open."""
        self.connected = True
        self.last_pong_time = time.time()
        
        if self.on_connect:
            try:
                self.on_connect()
            except Exception as e:
                logger.error(f"Connect callback error: {e}")

    def _on_ws_message(self, ws, message):
        """Handle incoming WebSocket messages."""
        try:
            data = json.loads(message)
            
            # Handle subscription responses
            if "status" in data:
                status = data.get("status")
                symbol = data.get("symbol")
                msg = data.get("message", "")
                
                if status != "success":
                    logger.error(f"Subscription error - {msg} - Symbol: {symbol}")
            
            # Handle market data
            elif "symbol" in data and "data" in data:
                tick_data = {
                    "symbol": data["symbol"],
                    "data": data["data"],
                    "timestamp": data.get("timestamp", ""),
                    "received_at": datetime.now()
                }
                
                if self.on_ticks:
                    try:
                        self.on_ticks(tick_data)
                    except Exception as e:
                        logger.error(f"Ticks callback error: {str(e)}")
                
        except json.JSONDecodeError:
            logger.error(f"Failed to parse message: {message}")
        except Exception as e:
            logger.error(f"Message handling error: {e}")

    def _on_ws_error(self, ws, error):
        """Handle WebSocket errors."""
        logger.error(f"WebSocket error: {error}")
        
        if self.on_error:
            try:
                self.on_error(error)
            except Exception as e:
                logger.error(f"Error callback failed: {e}")

    def _on_ws_close(self, ws, close_status_code, close_msg):
        """Handle WebSocket connection close."""
        self.connected = False
        
        if self.on_disconnect:
            try:
                self.on_disconnect()
            except Exception as e:
                logger.error(f"Disconnect callback error: {e}")
        
        # Auto-reconnect if enabled
        if self.auto_reconnect and self.running:
            self._attempt_reconnect()

    def _on_ws_ping(self, ws, data):
        """Handle incoming ping from server."""
        pass

    def _on_ws_pong(self, ws, data):
        """Handle incoming pong from server."""
        self.last_pong_time = time.time()

    def _attempt_reconnect(self):
        """Attempt to reconnect to WebSocket."""
        if self.reconnect_attempts >= self.max_reconnect_attempts:
            logger.error(f"Max reconnect attempts ({self.max_reconnect_attempts}) reached")
            self._stop_event.set()
            return
            
        self.reconnect_attempts += 1
        logger.info(f"Attempting to reconnect ({self.reconnect_attempts}/{self.max_reconnect_attempts})")
        
        time.sleep(self.reconnect_interval)
        
        if self.ws_connect():
            # Re-subscribe to current symbols if any
            if self.subscribed_symbols:
                symbols_to_resubscribe = list(self.subscribed_symbols)
                self.subscribed_symbols.clear()
                self.subscribe_multiple(symbols_to_resubscribe)

    def _run_websocket_with_ping(self):
        """Run WebSocket with ping/pong in an interruptible way."""
        try:
            while self.running and not self._stop_event.is_set():
                try:
                    self.ws.run_forever(
                        ping_interval=self.heartbeat_interval,
                        ping_timeout=self.ping_timeout,
                        ping_payload="ping"
                    )
                    break
                except Exception as e:
                    if self._stop_event.is_set() or not self.running:
                        break
                    logger.error(f"WebSocket error, retrying: {e}")
                    if self.auto_reconnect:
                        time.sleep(1)
                    else:
                        break
        except Exception as e:
            if not self._stop_event.is_set():
                logger.error(f"WebSocket thread error: {e}")

    def _run_websocket_simple(self):
        """Run WebSocket without ping in an interruptible way."""
        try:
            while self.running and not self._stop_event.is_set():
                try:
                    self.ws.run_forever()
                    break
                except Exception as e:
                    if self._stop_event.is_set() or not self.running:
                        break
                    logger.error(f"WebSocket error, retrying: {e}")
                    if self.auto_reconnect:
                        time.sleep(1)
                    else:
                        break
        except Exception as e:
            if not self._stop_event.is_set():
                logger.error(f"WebSocket thread error: {e}")

    def force_shutdown(self):
        """Force immediate shutdown of all components."""
        # Set all stop flags immediately
        self._stop_event.set()
        self.running = False
        self.auto_reconnect = False
        self.connected = False
        
        # Force close WebSocket without waiting
        if self.ws:
            try:
                self.ws.close()
                self.ws.keep_running = False
            except:
                pass
        
        # Don't wait for thread - just mark it as done
        self.ws = None
        self.ws_thread = None
        self.subscribed_symbols.clear()

    # Utility methods
    def is_connected(self) -> bool:
        """Check if WebSocket is connected."""
        return self.connected

    def get_subscribed_symbols(self) -> Set[str]:
        """Get set of currently subscribed symbols."""
        return self.subscribed_symbols.copy()

    def is_subscribed(self, symbol: str) -> bool:
        """Check if subscribed to a specific symbol."""
        return symbol.upper() in self.subscribed_symbols

    def get_subscription_count(self) -> int:
        """Get the number of active subscriptions."""
        return len(self.subscribed_symbols)
