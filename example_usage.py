#!/usr/bin/env python3
"""
NSE Socket Client - Example Usage
Demonstrates how to use the NSE client library similar to the breeze API pattern.
"""

import time
import signal
import sys
from nse_client import NSEClient, create_client

# Global client variable for signal handling
client = None

def signal_handler(sig, frame):
    """Handle Ctrl+C gracefully"""
    print("\n🛑 Shutting down...")
    if client:
        client.ws_disconnect()
    sys.exit(0)

def main():
    global client
    
    print("🚀 NSE Socket Client - Example Usage")
    print("=" * 50)
    
    # Method 1: Create client with explicit parameters (similar to breeze)
    client = NSEClient(
        uri="localhost",
        token="eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZG1pbiIsImp0aSI6IjgyN2MyY2RhLTZjNzktNDljMi05ZTRlLWI1ZjJmOTg0MzJiMCIsImV4cCI6MTgxMDQyNjQ3NSwiaWF0IjoxNzUwNDI2NDc1LCJ1c2VyX2lkIjoiYWRtaW4iLCJwZXJtaXNzaW9ucyI6WyJyZWFkX2RhdGEiLCJ3ZWJzb2NrZXRfY29ubmVjdCIsImFkbWluIl19.BNhcBJB8ytYbOrhWqk4EjTPHtwomZWnP86kpcMc2wy4"  # Replace with your token
    )
    
    # Method 2: Create client with convenience function (auto-loads token)
    # client = create_client()
    
    # Set up signal handler for graceful shutdown
    signal.signal(signal.SIGINT, signal_handler)
    
    # ================================================================
    # CALLBACKS - Define your event handlers (similar to breeze pattern)
    # ================================================================
    
    def on_ticks(ticks):
        """Callback to receive real-time stock data ticks."""
        symbol = ticks["symbol"]
        data = ticks["data"]
        timestamp = ticks["timestamp"]
        
        print(f"📊 {symbol} | "
              f"Close: ₹{data['close']:.2f} | "
              f"Volume: {data['volume']:,} | "
              f"High: ₹{data['high']:.2f} | "
              f"Low: ₹{data['low']:.2f}")
    
    def on_connect():
        """Callback when WebSocket connects."""
        print("✅ Connected to NSE Socket server!")
        print("📡 Ready to receive real-time data...")
    
    def on_disconnect():
        """Callback when WebSocket disconnects."""
        print("❌ Disconnected from NSE Socket server")
    
    def on_error(error):
        """Callback for WebSocket errors."""
        print(f"❌ WebSocket Error: {error}")
    
    def on_order_update(order):
        """Callback for order status updates."""
        status = order["status"]
        symbol = order["symbol"]
        side = order["side"]
        quantity = order["quantity"]
        
        print(f"📦 Order Update: {status.upper()} - {side.upper()} {quantity} {symbol}")
    
    # ================================================================
    # ASSIGN CALLBACKS (similar to breeze pattern)
    # ================================================================
    
    client.on_ticks = on_ticks
    client.on_connect = on_connect
    client.on_disconnect = on_disconnect  
    client.on_error = on_error
    client.on_order_update = on_order_update
    
    # ================================================================
    # CONNECTION MANAGEMENT (similar to breeze pattern)
    # ================================================================
    
    print("\n🔌 Connecting to WebSocket...")
    if not client.ws_connect():
        print("❌ Failed to connect. Is the server running?")
        return
    
    # Wait for connection to stabilize
    time.sleep(2)
    
    # ================================================================
    # SUBSCRIPTION MANAGEMENT
    # ================================================================
    
    print("\n📡 Testing multiple feed subscriptions...")
    
    # Subscribe to multiple symbols at once
    print("📊 Subscribing to multiple symbols...")
    symbols_to_subscribe = ["NIFTY", "INDIGO", "RELIANCE", "TCS"]
    results = client.subscribe_multiple(symbols_to_subscribe)
    
    for symbol, success in results.items():
        if success:
            print(f"✅ Successfully subscribed to {symbol}")
        else:
            print(f"❌ Failed to subscribe to {symbol}")
    
    time.sleep(10)  # Let data come through for multiple symbols
    
    # Show current subscriptions
    current_subscriptions = client.get_subscribed_symbols()
    print(f"📊 Currently subscribed to {len(current_subscriptions)} symbols: {', '.join(current_subscriptions)}")
    
    # Unsubscribe from specific symbols
    print("📊 Unsubscribing from INDIGO and TCS...")
    client.unsubscribe_multiple(["INDIGO", "TCS"])
    time.sleep(3)
    
    # Check remaining subscriptions
    remaining_subscriptions = client.get_subscribed_symbols()
    print(f"📊 Remaining subscriptions: {', '.join(remaining_subscriptions)}")
    
    # Add one more symbol individually
    print("📊 Adding WIPRO individually...")
    client.subscribe_feed("WIPRO")
    time.sleep(5)
    
    print(f"📊 Total active subscriptions: {client.get_subscription_count()}")
    
    # Unsubscribe from all
    print("📊 Unsubscribing from all symbols...")
    client.unsubscribe_feed()  # Unsubscribe from all
    time.sleep(2)
    
    # ================================================================
    # ORDER MANAGEMENT
    # ================================================================
    
    print("\n📦 Testing order management...")
    
    # Check API health first
    if client.health_check():
        print("✅ API server is healthy")
    else:
        print("❌ API server is not responding")
        client.ws_disconnect()
        return
    
    # Place different types of orders
    orders_placed = []
    
    # 1. Market Buy Order
    print("\n📦 Placing market buy order...")
    order = client.place_order(
        symbol="NIFTY",
        side="buy", 
        order_type="market",
        quantity=100
    )
    if order:
        orders_placed.append(order["id"])
        print(f"✅ Market order placed: {order['id']}")
    
    time.sleep(1)
    
    # 2. Limit Sell Order
    print("\n📦 Placing limit sell order...")
    order = client.place_order(
        symbol="INDIGO",
        side="sell",
        order_type="limit", 
        quantity=50,
        price=1250.75
    )
    if order:
        orders_placed.append(order["id"])
        print(f"✅ Limit order placed: {order['id']}")
    
    time.sleep(1)
    
    # 3. Stop Loss Order
    print("\n📦 Placing stop-loss order...")
    order = client.place_order(
        symbol="NIFTY",
        side="sell",
        order_type="stop_loss",
        quantity=200, 
        stop_price=21000.0
    )
    if order:
        orders_placed.append(order["id"])
        print(f"✅ Stop-loss order placed: {order['id']}")
    
    time.sleep(1)
    
    # ================================================================
    # ORDER RETRIEVAL
    # ================================================================
    
    print("\n📋 Retrieving orders...")
    
    # Get all orders
    all_orders = client.get_orders()
    print(f"📊 Total orders: {len(all_orders)}")
    
    # Get orders by symbol
    nifty_orders = client.get_orders(symbol="NIFTY")
    print(f"📊 NIFTY orders: {len(nifty_orders)}")
    
    # Get pending orders only
    pending_orders = client.get_orders(status="pending")
    print(f"📊 Pending orders: {len(pending_orders)}")
    
    # Get specific order details
    if orders_placed:
        order_id = orders_placed[0]
        order_details = client.get_order(order_id)
        if order_details:
            print(f"📄 Order {order_id[:8]}... details retrieved")
    
    # ================================================================
    # ORDER CANCELLATION
    # ================================================================
    
    print("\n🚫 Testing order cancellation...")
    
    # Cancel first order if any
    if orders_placed:
        order_id = orders_placed[0]
        print(f"🚫 Cancelling order: {order_id[:8]}...")
        if client.cancel_order(order_id):
            print("✅ Order cancelled successfully")
        else:
            print("❌ Failed to cancel order")
    
    time.sleep(1)
    
    # ================================================================
    # FINAL DATA STREAMING TEST
    # ================================================================
    
    print("\n📡 Final data streaming test...")
    print("🔄 Subscribing to NIFTY for final data stream...")
    client.subscribe_feed("NIFTY")
    
    print("⏱️  Streaming data for 10 seconds... (Press Ctrl+C to stop)")
    try:
        time.sleep(10)
    except KeyboardInterrupt:
        pass
    
    # ================================================================
    # CLEANUP
    # ================================================================
    
    print("\n🧹 Cleaning up...")
    
    # Show final order summary
    final_orders = client.get_orders()
    pending_count = len([o for o in final_orders if o["status"] == "pending"])
    cancelled_count = len([o for o in final_orders if o["status"] == "cancelled"])
    
    print(f"📊 Final Summary:")
    print(f"   Total Orders: {len(final_orders)}")
    print(f"   Pending: {pending_count}")
    print(f"   Cancelled: {cancelled_count}")
    
    # Disconnect from WebSocket
    client.ws_disconnect()
    print("✅ Example completed successfully!")


def simple_streaming_example():
    """Simple example for just data streaming"""
    print("📡 Simple Streaming Example")
    print("=" * 30)
    
    # Create client
    client = create_client()
    
    # Simple tick handler
    def on_ticks(ticks):
        data = ticks["data"]
        print(f"📊 {ticks['symbol']}: ₹{data['close']:.2f}")
    
    # Set callback and connect
    client.on_ticks = on_ticks
    
    if client.ws_connect():
        client.subscribe_feed("NIFTY")
        print("📡 Streaming NIFTY data for 15 seconds...")
        time.sleep(15)
        client.ws_disconnect()


def simple_trading_example():
    """Simple example for just order management"""
    print("📦 Simple Trading Example") 
    print("=" * 30)
    
    # Create client
    client = create_client()
    
    # Order update handler
    def on_order_update(order):
        print(f"📦 {order['status'].upper()}: {order['side'].upper()} {order['quantity']} {order['symbol']}")
    
    client.on_order_update = on_order_update
    
    # Place and manage orders
    if client.health_check():
        print("✅ API is healthy")
        
        # Place order
        order = client.place_order("NIFTY", "buy", "market", 50)
        if order:
            order_id = order["id"]
            print(f"📦 Order placed: {order_id[:8]}...")
            
            time.sleep(2)
            
            # Cancel order
            client.cancel_order(order_id)
            
            # Check final status
            final_order = client.get_order(order_id)
            if final_order:
                print(f"📊 Final status: {final_order['status']}")


if __name__ == "__main__":
    print("Choose an example to run:")
    print("1. Full example (streaming + trading)")
    print("2. Simple streaming only")
    print("3. Simple trading only")
    
    try:
        choice = input("\nEnter choice (1-3): ").strip()
        
        if choice == "1":
            main()
        elif choice == "2":
            simple_streaming_example()
        elif choice == "3":
            simple_trading_example()
        else:
            print("Invalid choice, running full example...")
            main()
            
    except KeyboardInterrupt:
        print("\n👋 Goodbye!")
    except Exception as e:
        print(f"❌ Error: {e}")
        if client:
            client.ws_disconnect() 