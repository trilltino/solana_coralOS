# Coral docker-runtime image for the helius-monitor agent.
# Coral Server launches this container and injects CORAL_CONNECTION_URL plus the
# options declared in coral-agent.toml as environment variables.
FROM python:3.13-slim

WORKDIR /app
RUN pip install --no-cache-dir "websockets>=12.0" "mcp>=1.0.0"

COPY coral_agent.py /app/coral_agent.py

ENTRYPOINT ["python", "/app/coral_agent.py"]
