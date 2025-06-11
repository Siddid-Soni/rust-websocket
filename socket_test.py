import time
import signal
import sys
from nse_client import NSEClient, create_client
from collections import deque

client = None

def signal_handler(sig, frame):
    """Handle Ctrl+C gracefully"""
    print("\n🛑 Shutting down...")
    if client:
        client.ws_disconnect()
    sys.exit(0)

client = NSEClient(
        ws_uri="ws://localhost:8080/ws",
        api_uri="http://localhost:3000",
        token="eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyMTIzIiwianRpIjoiMTNiYWJlN2ItYWI1NC00NzE1LTkyMjctYWY3OWJmYTU2YmRkIiwiZXhwIjoxNzY3MDU2MzI3LCJpYXQiOjE3NDk2NTYzMjcsInVzZXJfaWQiOiJ1c2VyMTIzIiwicGVybWlzc2lvbnMiOlsicmVhZF9kYXRhIiwid2Vic29ja2V0X2Nvbm5lY3QiXX0.aeG7R56atD9trvwreIMca0UVnBHFJQeON1kMpjVtz3s"  # Replace with your token
)

q = deque(maxlen=5)

def on_ticks(ticks):
        """Callback to receive real-time stock data ticks."""
        q.append(ticks["data"]["close"])
        
        if len(q) == 5:
              print(sum(q)/5, q)

        symbol = ticks["symbol"]
        data = ticks["data"]
        timestamp = ticks["timestamp"]
        
        print(f"📊 {symbol} | "
              f"Close: ₹{data['close']:.2f} | "
              f"Volume: {data['volume']:,} | "
              f"High: ₹{data['high']:.2f} | "
              f"Low: ₹{data['low']:.2f}")
        
def on_order_update(order):
        """Callback for order status updates."""
        status = order["status"]
        symbol = order["symbol"]
        side = order["side"]
        quantity = order["quantity"]
        
        print(f"📦 Order Update: {status.upper()} - {side.upper()} {quantity} {symbol}")


client.on_ticks = on_ticks
client.on_order_update = on_order_update

client.ws_connect()
results = client.subscribe_multiple(["NIFTY", "INDIGO"])

time.sleep(100)