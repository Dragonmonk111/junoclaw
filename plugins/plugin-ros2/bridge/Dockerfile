FROM python:3.11-slim

WORKDIR /app

COPY pyproject.toml .
COPY src/ ./src/
COPY README.md .

RUN pip install --no-cache-dir -e ".[simulate]"

EXPOSE 8080

CMD ["junoclaw-ros2-bridge", "--robot-id", "robot-01", "--simulate", "--port", "8080"]
