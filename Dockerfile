# Build backend
FROM rust:1.83-slim-bookworm as builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release --bin zeroclaw

# Build frontend
FROM node:20-slim as frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm install
COPY frontend/ .
RUN npm run build

# Final image
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/zeroclaw /app/zeroclaw
COPY --from=builder /app/migrations /app/migrations
COPY --from=frontend-builder /app/frontend/dist /app/public

ENV ZEROCLAW_CONFIG_PATH=/app/config.toml
ENV PUBLIC_DIR=/app/public

CMD ["/app/zeroclaw", "daemon", "--host", "0.0.0.0", "--port", "8080"]
