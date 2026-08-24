#!/usr/bin/env python3
"""
Simple WebSocket Client for NSE Socket Server

A minimal example showing how to connect to the NSE Socket server using websocket-client.
This is perfect for getting started quickly or integrating into existing applications.

Prerequisites:
- pip install websocket-client requests
- NSE Socket server running

Usage:
    python simple_websocket_client.py
"""

import json
import time
import requests
import websocket
from datetime import datetime


def get_auth_token(username="test_user", api_url="http://localhost:3000/api"):
    """Get JWT authentication token."""
    try:
        response = requests.post(f"{api_url}/login", json={"username": username})
        if response.status_code == 200:
            data = response.json()
            if data.get("success"):
                print(f"✅ Authenticated as: {username}")
                return data.get("token")
        print(f"❌ Authentication failed: {response.status_code}")
        return None
    except Exception as e:
        print(f"❌ Authentication error: {e}")
        return None


def on_message(ws, message):
    """Handle incoming messages."""
    try:
        data = json.loads(message)
        
        # Stock data message
        if data.get("symbol") and data.get("data"):
            print(f"\n📊 {data['symbol']} Update:")
            stock_data = data['data']
            print(f"   Price: ${stock_data['close']:.2f}")
            print(f"   Volume: {stock_data['volume']:,}")
            print(f"   Date: {stock_data['date']}")
            
            # Show scaled timestamp if available
            if stock_data.get('scaled_timestamp'):
                scaled_time = datetime.fromisoformat(stock_data['scaled_timestamp'].replace('Z', '+00:00'))
                print(f"   Scaled Time: {scaled_time.strftime('%H:%M:%S')}")
        
        # Subscription response (success/error)
        elif data.get("status"):
            status = data['status']
            symbol = data.get('symbol', 'Unknown')
            message_text = data.get('message', '')
            
            if status == "success":
                print(f"✅ {message_text} for {symbol}")
            elif status == "error":
                print(f"❌ {message_text} for {symbol}")
            else:
                print(f"📨 Status: {status} for {symbol} - {message_text}")
        
        # Legacy subscription confirmation
        elif data.get("type") == "subscription_confirmed":
            print(f"✅ Subscribed to: {data.get('symbol')}")
            
        # Legacy error message
        elif data.get("type") == "error":
            print(f"❌ Error: {data.get('message')}")
            
        else:
            print(f"📨 Message: {message}")
            
    except json.JSONDecodeError:
        print(f"📨 Raw message: {message}")
    except KeyError as e:
        print(f"❌ Missing key in message: {e}")
        print(f"📨 Full message: {message}")
    except Exception as e:
        print(f"❌ Error processing message: {e}")
        print(f"📨 Full message: {message}")


def on_error(ws, error):
    """Handle errors."""
    print(f"❌ WebSocket error: {error}")


def on_close(ws, close_status_code, close_msg):
    """Handle connection close."""
    print(f"🔌 Connection closed: {close_status_code}")


def on_open(ws):
    """Handle connection open."""
    print("🔗 WebSocket connected!")
    
    # Subscribe to BHEL data automatically
    subscribe_msg = {
        "action": "subscribe", 
        "symbol": "BHEL"
    }
    ws.send(json.dumps(subscribe_msg))
    print("📡 Subscribing to BHEL data...")


def main():
    """Simple WebSocket client example."""
    print("🚀 Simple NSE WebSocket Client")
    print("=" * 40)
    
    # Step 1: Get authentication token
    token = get_auth_token()
    if not token:
        print("❌ Failed to authenticate")
        return
    
    # Step 2: Connect to WebSocket
    ws_url = "ws://localhost:8080/ws"
    headers = [f"Authorization: Bearer {token}"]
    
    print(f"🔗 Connecting to: {ws_url}")
    
    ws = websocket.WebSocketApp(
        ws_url,
        header=headers,
        on_open=on_open,
        on_message=on_message,
        on_error=on_error,
        on_close=on_close
    )
    
    # Step 3: Run WebSocket (this blocks)
    print("👂 Listening for data... (Press Ctrl+C to stop)")
    try:
        ws.run_forever()
    except KeyboardInterrupt:
        print("\n⏹️  Disconnecting...")
        ws.close()


# Advanced example with manual subscription management
def advanced_example():
    """Advanced WebSocket client with manual control."""
    print("\n🎯 Advanced WebSocket Client Example")
    print("=" * 50)
    
    # Get token
    token = get_auth_token("advanced_user")
    if not token:
        return
    
    # Connect
    ws = websocket.create_connection(
        "ws://localhost:8080/ws",
        header=[f"Authorization: Bearer {token}"]
    )
    
    print("🔗 Connected successfully!")
    
    try:
        # Subscribe to multiple symbols
        symbols = ["BHEL", "ABB"]
        
        for symbol in symbols:
            subscribe_msg = {"action": "subscribe", "symbol": symbol}
            ws.send(json.dumps(subscribe_msg))
            print(f"📡 Subscribed to {symbol}")
            time.sleep(0.5)  # Small delay
        
        # Listen for messages
        message_count = 0
        max_messages = 10  # Stop after 10 messages
        
        print(f"\n👂 Listening for {max_messages} messages...")
        
        while message_count < max_messages:
            try:
                # Set timeout to avoid hanging
                ws.settimeout(30)
                message = ws.recv()
                
                data = json.loads(message)
                
                # Check for stock data with proper error handling
                if data.get("symbol") and data.get("data"):
                    message_count += 1
                    symbol = data['symbol']
                    price = data['data']['close']
                    print(f"{message_count:2d}. {symbol}: ${price:.2f}")
                elif data.get("status"):
                    # Handle subscription confirmations
                    status = data['status']
                    symbol = data.get('symbol', 'Unknown')
                    print(f"📨 {status}: {symbol}")
                else:
                    print(f"📨 Other message: {message}")
                    
            except websocket.WebSocketTimeoutException:
                print("⏰ Timeout waiting for message")
                break
            except json.JSONDecodeError:
                print(f"📨 Non-JSON message: {message}")
            except KeyError as e:
                print(f"❌ Missing key {e} in message: {message}")
            except Exception as e:
                print(f"❌ Error processing message: {e}")
                break
        
        # Unsubscribe from symbols
        for symbol in symbols:
            unsubscribe_msg = {"action": "unsubscribe", "symbol": symbol}
            ws.send(json.dumps(unsubscribe_msg))
            print(f"🔕 Unsubscribed from {symbol}")
            time.sleep(0.2)
        
        print(f"✅ Received {message_count} stock updates")
        
    except Exception as e:
        print(f"❌ Error: {e}")
    finally:
        ws.close()
        print("🔌 Connection closed")


# Callback-based example for integration
class StockDataCallback:
    """Example callback handler for stock data."""
    
    def __init__(self):
        self.data_count = 0
        self.latest_prices = {}
    
    def on_stock_update(self, symbol, price, volume, date):
        """Called when new stock data arrives."""
        self.data_count += 1
        self.latest_prices[symbol] = price
        
        print(f"📈 {symbol}: ${price:.2f} (Volume: {volume:,})")
        
        # Example: Alert on significant volume
        if volume > 1000000:
            print(f"🚨 High volume alert for {symbol}: {volume:,}")
    
    def get_summary(self):
        """Get summary of received data."""
        return {
            "total_updates": self.data_count,
            "symbols_tracked": len(self.latest_prices),
            "latest_prices": self.latest_prices.copy()
        }


def callback_example():
    """Example using callbacks for easier integration."""
    print("\n🔄 Callback-based WebSocket Client")
    print("=" * 40)
    
    # Create callback handler
    callback = StockDataCallback()
    
    # Get token
    token = get_auth_token("callback_user")
    if not token:
        return
    
    def on_message_callback(ws, message):
        try:
            data = json.loads(message)
            if data.get("symbol"):
                symbol = data['symbol']
                stock_data = data['data']
                callback.on_stock_update(
                    symbol=symbol,
                    price=stock_data['close'],
                    volume=stock_data['volume'],
                    date=stock_data['date']
                )
        except json.JSONDecodeError:
            pass
    
    def on_open_callback(ws):
        print("🔗 Connected with callbacks!")
        # Subscribe to symbols
        for symbol in ["BHEL", "ABB"]:
            ws.send(json.dumps({"action": "subscribe", "symbol": symbol}))
    
    # Create WebSocket with callbacks
    ws = websocket.WebSocketApp(
        "ws://localhost:8080/ws",
        header=[f"Authorization: Bearer {token}"],
        on_open=on_open_callback,
        on_message=on_message_callback,
        on_error=lambda ws, error: print(f"❌ Error: {error}"),
        on_close=lambda ws, code, msg: print("🔌 Disconnected")
    )
    
    # Run for limited time
    print("👂 Running callback example for 20 seconds...")
    
    def stop_after_delay():
        time.sleep(20)
        ws.close()
    
    import threading
    timer = threading.Thread(target=stop_after_delay, daemon=True)
    timer.start()
    
    try:
        ws.run_forever()
    except:
        pass
    
    # Show summary
    summary = callback.get_summary()
    print(f"\n📊 Summary:")
    print(f"   Total updates: {summary['total_updates']}")
    print(f"   Symbols tracked: {summary['symbols_tracked']}")
    print(f"   Latest prices: {summary['latest_prices']}")


if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1:
        if sys.argv[1] == "advanced":
            advanced_example()
        elif sys.argv[1] == "callback":
            callback_example()
        else:
            print("Usage: python simple_websocket_client.py [advanced|callback]")
    else:
        # Run simple example by default
        main() 