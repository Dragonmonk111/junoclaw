"""Quick launcher: start the bridge in simulate mode, accessible on LAN."""
from junoclaw_ros2_bridge.server import create_app
import uvicorn

app = create_app(robot_id="demo-bot", simulate=True)
uvicorn.run(app, host="0.0.0.0", port=8000)
