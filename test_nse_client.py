#!/usr/bin/env python3
"""
Test subscribe_feed function - background subscription
"""

import time
from nse_client import NSEClient, get_token

print("TESTING SUBSCRIBE FEED FUNCTION")
print("="*40)

# Get token and create client
token = get_token("localhost", "admin")
client = NSEClient("localhost", token)

# Set up message handler
messages = []
def on_message(data):
    messages.append(data)
    symbol = data.get("symbol", "Unknown")
    close_price = data.get("data", {}).get("close", "N/A")
    print(f"📊 {symbol}: {close_price}")

client.on_ticks = on_message

# Test subscribe_feed BEFORE connection (should queue)
print("Testing subscribe_feed('BHEL') before connection...")
success1 = client.subscribe_feed("BHEL")
print(f"Subscribe result (queued): {success1}")

# Start WebSocket connection in background
print("Starting WebSocket connection...")
client.ws_connect(blocking=False)

# Test another subscription while connecting
print("Testing subscribe_feed('ABB') while connecting...")
success2 = client.subscribe_feed("ABB")
print(f"Subscribe result (queued): {success2}")

# Wait for data
print("Waiting 15 seconds for connection and data...")
time.sleep(15)

print(f"Connection status: {client.connected}")
print(f"Messages received: {len(messages)}")
print(f"Subscribed symbols: {client.get_subscribed_symbols()}")

# Cleanup
client.stop()
print("Test complete") 