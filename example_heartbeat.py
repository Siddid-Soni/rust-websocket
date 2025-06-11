#!/usr/bin/env python3
"""
NSE Client - Heartbeat Example
Demonstrates the heartbeat/ping functionality to keep connections alive.
"""

from nse_client import create_client
import time
import threading

def main():
    print("💓 NSE Client Heartbeat Example")
    print("=" * 40)
    
    # Create client
    client = create_client(
        ws_uri="ws://localhost:8080/ws",
    )
    
    # Configure heartbeat (optional - it's enabled by default)
    client.set_heartbeat_config(
        enabled=True,    # Enable heartbeat
        interval=20,     # Ping every 20 seconds (server expects within 30s)
        timeout=8        # Wait 8 seconds for pong response
    )
    
    # Set up callbacks
    def on_ticks(ticks):
        symbol = ticks["symbol"]
        data = ticks["data"]
        timestamp = ticks.get("timestamp", "")
        print(f"📊 {symbol}: ${data['close']:.2f} [{timestamp}]")
    
    def on_connect():
        print("✅ Connected to NSE Socket server")
        print(f"💓 Heartbeat status: {client.get_heartbeat_status()}")
    
    def on_disconnect():
        print("❌ Disconnected from server")
    
    def on_error(error):
        print(f"❌ Error: {error}")
    
    # Assign callbacks
    client.on_ticks = on_ticks
    client.on_connect = on_connect
    client.on_disconnect = on_disconnect
    client.on_error = on_error
    
    # Connect and subscribe
    symbols = ["NIFTY", "INDIGO"]
    
    if client.connect_and_subscribe(symbols):
        print("\n🚀 Starting data stream with heartbeat...")
        print("💡 Heartbeat will keep the connection alive automatically")
        print("💡 Check server logs to see ping/pong messages")
        print("-" * 50)
        
        # Start a thread to periodically show heartbeat status
        def show_status():
            while client.connected:
                time.sleep(30)  # Show status every 30 seconds
                if client.connected:
                    status = client.get_heartbeat_status()
                    print(f"\n💓 Heartbeat Status: {status}")
        
        status_thread = threading.Thread(target=show_status, daemon=True)
        status_thread.start()
        
        # Run until interrupted
        client.run()
    else:
        print("❌ Failed to connect and subscribe")


def test_heartbeat_failure():
    """Test what happens when heartbeat fails (simulated network issue)"""
    print("\n🧪 Testing Heartbeat Failure Handling")
    print("=" * 40)
    
    client = create_client()
    
    # Configure aggressive heartbeat for testing
    client.set_heartbeat_config(
        enabled=True,
        interval=5,      # Ping every 5 seconds
        timeout=3        # Only wait 3 seconds for pong
    )
    
    def on_connect():
        print("✅ Connected with aggressive heartbeat settings")
    
    def on_disconnect():
        print("❌ Disconnected - heartbeat failure or network issue")
    
    client.on_connect = on_connect
    client.on_disconnect = on_disconnect
    
    if client.ws_connect():
        print("🔄 Running for 60 seconds with aggressive heartbeat...")
        print("💡 If network is unstable, you may see reconnection attempts")
        
        try:
            client.run(timeout=60)
        except KeyboardInterrupt:
            print("🛑 Test interrupted")
        
        client.ws_disconnect()
    else:
        print("❌ Failed to connect")


def test_without_heartbeat():
    """Test connection without heartbeat (not recommended for production)"""
    print("\n🚫 Testing Without Heartbeat")
    print("=" * 30)
    
    client = create_client()
    
    # Disable heartbeat
    client.set_heartbeat_config(enabled=False)
    
    def on_connect():
        print("✅ Connected WITHOUT heartbeat")
        print("⚠️ Connection may become stale without server knowing")
    
    client.on_connect = on_connect
    
    if client.connect_and_subscribe(["NIFTY"]):
        print("🔄 Running for 30 seconds without heartbeat...")
        client.run(timeout=30)
    else:
        print("❌ Failed to connect")


if __name__ == "__main__":
    try:
        # Run the main heartbeat example
        main()
        
        # Uncomment to test other scenarios:
        # test_heartbeat_failure()
        # test_without_heartbeat()
        
    except KeyboardInterrupt:
        print("\n🛑 Interrupted by user")
    except Exception as e:
        print(f"❌ Error: {e}") 