#!/usr/bin/env python3
"""
Updated NSE Client Example - Simplified Non-Blocking API
"""

from nse_client import NSEClient, get_token
import time

def main():
    print("🚀 NSE Socket Client - Simplified Non-Blocking API")
    print("=" * 55)
    
    # Method 1: Use standalone get_token function
    print("\n1️⃣ Getting token using standalone function:")
    token = get_token("http://localhost:3000", "admin")
    if token:
        print(f"✅ Token obtained: {token[:50]}...")
    else:
        print("❌ Failed to get token")
        return
    
    # Create client with token
    client = NSEClient("ws://localhost:8080", "http://localhost:3000", token)
    
    # Method 2: Test single historical data function (clean format)
    print("\n2️⃣ Testing historical data function:")
    try:
        # Get historical data - now returns clean format directly
        historical_data = client.get_historical_data("INDIGO", limit=5)
        if historical_data:
            print(f"✅ Historical data: {len(historical_data)} records")
            for record in historical_data[:2]:  # Show first 2
                print(f"   📊 {record.get('date')}: Close={record.get('close')}")
        else:
            print("❌ No historical data received")
    except Exception as e:
        print(f"❌ Historical data error: {e}")
    
    # Method 3: Test date range filtering
    print("\n3️⃣ Testing date range filtering:")
    try:
        filtered_data = client.get_historical_data("INDIGO", limit=3, from_date="2025-06-19")
        if filtered_data:
            print(f"✅ Filtered data: {len(filtered_data)} records")
            for record in filtered_data:
                print(f"   📈 {record['date']}: OHLCV = {record['open']}, {record['high']}, {record['low']}, {record['close']}, {record['volume']}")
    except Exception as e:
        print(f"❌ Filtered data error: {e}")
    
    # Method 4: Simplified streaming API
    print("\n4️⃣ Testing simplified streaming API:")
    
    def handle_streaming_data(data):
        symbol = data["symbol"]
        price_data = data["data"]
        timestamp = data.get("timestamp", "")
        print(f"📡 {symbol}: {price_data['close']:.2f} | Vol: {price_data['volume']:,} | {timestamp[:19]}")
    
    # Set callback via property (cleaner API)
    client.on_ticks = handle_streaming_data
    
    print("🚀 Starting streaming with simplified API:")
    print("💡 client.on_ticks = callback, then client.start_streaming(symbols)")
    print("🛑 Press Ctrl+C to stop")
    
    # Simple streaming - no parameters needed for callback
    client.start_streaming(["INDIGO", "ABB"])
    
    # Program will stay alive until Ctrl+C or client.stop()
    print("✅ Streaming finished")

if __name__ == "__main__":
    main()