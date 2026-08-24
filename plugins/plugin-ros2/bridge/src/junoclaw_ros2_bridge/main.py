"""CLI entry point for the JunoClaw ROS2 bridge."""

import argparse
import sys

from .server import create_app


def main():
    parser = argparse.ArgumentParser(
        description="JunoClaw ROS2 Bridge — HTTP bridge for the JunoClaw trust stack"
    )
    parser.add_argument(
        "--robot-id",
        default="robot-01",
        help="Unique robot identifier (default: robot-01)",
    )
    parser.add_argument(
        "--simulate",
        action="store_true",
        help="Run in simulation mode (no ROS2 required)",
    )
    parser.add_argument(
        "--ros2-domain",
        type=int,
        default=0,
        help="ROS2 domain ID (default: 0)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8080,
        help="HTTP server port (default: 8080)",
    )
    parser.add_argument(
        "--host",
        default="0.0.0.0",
        help="HTTP server host (default: 0.0.0.0)",
    )
    parser.add_argument(
        "--robot-type",
        default="wheeled",
        choices=["wheeled", "quadruped"],
        help="Robot type: wheeled (default) or quadruped (12-DOF, e.g. DOGZILLA S2)",
    )
    parser.add_argument(
        "--sensor-topics",
        default=None,
        help="Comma-separated sensor topics (default: auto-selected based on robot-type)",
    )
    parser.add_argument(
        "--action-servers",
        default=None,
        help="Comma-separated action servers (default: auto-selected based on robot-type)",
    )

    args = parser.parse_args()

    if args.sensor_topics:
        sensor_topics = [t.strip() for t in args.sensor_topics.split(",")]
    else:
        sensor_topics = None  # let bridge auto-select based on robot_type
    if args.action_servers:
        action_servers = [s.strip() for s in args.action_servers.split(",")]
    else:
        action_servers = None  # let bridge auto-select based on robot_type

    app = create_app(
        robot_id=args.robot_id,
        simulate=args.simulate,
        ros2_domain=args.ros2_domain,
        sensor_topics=sensor_topics,
        action_servers=action_servers,
        robot_type=args.robot_type,
    )

    print(f"JunoClaw ROS2 Bridge starting...")
    print(f"  Robot ID: {args.robot_id}")
    print(f"  Robot type: {args.robot_type}")
    print(f"  Mode: {'SIMULATE' if args.simulate else 'ROS2'}")
    print(f"  Port: {args.port}")
    print(f"  Sensor topics: {sensor_topics or '(auto)'}")
    print(f"  Action servers: {action_servers or '(auto)'}")

    try:
        import uvicorn

        uvicorn.run(app, host=args.host, port=args.port, log_level="info")
    except ImportError:
        print("Error: uvicorn not installed. Run: pip install uvicorn[standard]")
        sys.exit(1)


if __name__ == "__main__":
    main()
