import websocket
import time

# WebSocket server URL
ws_url = "ws://127.0.0.1:8080"

# Define a function to measure latency
def measure_latency(start_time):
    latency = time.time() - start_time
    print(f"Latency: {latency} seconds")

# Define a function to handle incoming messages
def on_message(ws, message):
    print(f"Received: {message}")
    # Measure latency when message is received
    measure_latency(float(message))

# Define a function to handle WebSocket open event
def on_open(ws):
    print("Connected to WebSocket server")
    # Send a test message
    ws.send(str(time.time()))

# Create a WebSocket instance and bind event handlers
ws = websocket.WebSocketApp(ws_url,
                            header={"Authorization": "Bearer your-secret-token-123"},
                            on_message=on_message,
                            on_open=on_open)

# Connect to the WebSocket server
ws.run_forever()