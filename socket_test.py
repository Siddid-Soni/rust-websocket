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
        ws_uri="ws://157.157.221.30:46362/ws",
        api_uri="http://157.157.221.30:46363",
        token="eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhZG1pbiIsImp0aSI6ImU0YWQyOWU2LWRiOGQtNDZiNC1hZTc3LTEyMzUzNTkzNzkwNCIsImV4cCI6MTgxMDQyODcwOSwiaWF0IjoxNzUwNDI4NzA5LCJ1c2VyX2lkIjoiYWRtaW4iLCJwZXJtaXNzaW9ucyI6WyJyZWFkX2RhdGEiLCJ3ZWJzb2NrZXRfY29ubmVjdCIsImFkbWluIl19.hjg8U1p3pp2Dyk7mrU8kcVI0sgu9Tm50mnrj4yhHNuM"  # Replace with your token
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