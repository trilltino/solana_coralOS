# Coral docker-runtime image for the user-proxy agent.
# Coral Server launches this container and injects CORAL_CONNECTION_URL.
FROM python:3.13-slim

WORKDIR /app
RUN pip install --no-cache-dir "mcp>=1.0.0"

COPY agent.py /app/agent.py

ENTRYPOINT ["python", "/app/agent.py"]
