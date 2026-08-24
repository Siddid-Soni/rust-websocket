import time
import signal
import sys
from nse_socket_client import NSEClient, get_token
from collections import deque

token = get_token("94.136.185.170", "prawin")
# print(token)

client = NSEClient(
        uri="94.136.185.170",
        token=token  # Replace with your token
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

data = client.get_historical_data(symbol="ABB", time_period="minutes", from_date="2025-06-17", to_date="2025-06-17")
# Subscribe first (will be queued), then connect
limit_order = client.place_order(
        symbol="TCS",
        side="buy",
        order_type="limit", 
        quantity=5,
        price=3500.00
    )

client.place_order(
        symbol="BHEL",
        side="sell",
        order_type="limit", 
        quantity=5,
        price=3500.00
    )

client.place_order(
        symbol="BHEL",
        side="sell",
        order_type="limit", 
        quantity=5,
        price=3500.00
    )
client.place_order(
        symbol="BHEL",
        side="sell",
        order_type="limit", 
        quantity=5,
        price=3500.00
    )
print(data)
print(limit_order)

client.subscribe_multiple(["ABB", "BHEL"])
client.ws_connect()