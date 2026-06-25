# Human → user-proxy bridge (self-serves the Phantom checkout UI).
FROM node:20-slim
WORKDIR /app
COPY package.json ./
RUN npm install --omit=optional
COPY . .
EXPOSE 3010
CMD ["npx", "tsx", "server.ts"]
