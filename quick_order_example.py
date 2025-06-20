#!/usr/bin/env python3
"""
Quick Order Example - NSE Socket Client
=======================================

A simple example showing how to quickly place orders using the NSE client.
Perfect for getting started with order management.

Run this after starting your NSE Socket server.
"""

from nse_client import NSEClient


def quick_order_example():
    """Quick example of placing different types of orders."""
    
    print("NSE Socket Client - Quick Order Example")
    print("=" * 45)
    
    # 1. Create and authenticate client
    client = NSEClient("ws://157.157.221.30:46362", "http://157.157.221.30:46363")
    
    # Authenticate with username (will request JWT token)
    print("🔐 Authenticating...")
    if not client.authenticate("ij"):  # Change to your username
        print("❌ Authentication failed!")
        return
    
    print("✅ Authentication successful!")
    
    # 2. Place a market order
    print("\n📝 Placing market order...")
    market_order = client.place_order(
        symbol="RELIANCE",
        side="buy", 
        order_type="market",
        quantity=10
    )
    
    if market_order:
        print(f"✅ Market order placed! ID: {market_order['id'][:8]}...")
    
    # 3. Place a limit order
    print("\n📝 Placing limit order...")
    limit_order = client.place_order(
        symbol="TCS",
        side="sell",
        order_type="limit", 
        quantity=5,
        price=3500.00
    )
    
    if limit_order:
        print(f"✅ Limit order placed! ID: {limit_order['id'][:8]}...")
    
    # 4. Place a stop-loss order
    print("\n📝 Placing stop-loss order...")
    stop_order = client.place_order(
        symbol="HDFC",
        side="sell",
        order_type="stop_loss",
        quantity=8, 
        stop_price=1580.00
    )
    
    if stop_order:
        print(f"✅ Stop-loss order placed! ID: {stop_order['id'][:8]}...")
    
    # 5. View all orders
    print("\n📋 Checking all orders...")
    orders = client.get_orders()
    print(f"📊 Total orders: {len(orders)}")
    
    for order in orders:
        print(f"   • {order['symbol']} - {order['side'].upper()} {order['quantity']} - {order['status'].upper()}")
    
    # 6. Cancel the limit order (example)
    if limit_order and limit_order.get('status') == 'pending':
        print(f"\n🗑️ Canceling limit order...")
        if client.cancel_order(limit_order['id']):
            print("✅ Order cancelled successfully!")
    
    print("\n🎉 Quick order example completed!")


if __name__ == "__main__":
    try:
        quick_order_example()
    except KeyboardInterrupt:
        print("\n👋 Example interrupted by user")
    except Exception as e:
        print(f"\n❌ Error: {e}")
        print("Make sure your NSE Socket server is running!") 