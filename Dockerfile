FROM node:24-bookworm-slim AS frontend-build
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1.89-bookworm AS backend-build
WORKDIR /app
COPY backend/Cargo.toml ./backend/Cargo.toml
COPY backend/src/ ./backend/src/
COPY backend/migrations/ ./backend/migrations/
RUN cargo build --release --manifest-path backend/Cargo.toml

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend-build /app/backend/target/release/chat-rust-webrtc-api /usr/local/bin/chat-app
COPY --from=frontend-build /app/frontend/dist /app/frontend/dist
ENV STATIC_DIR=/app/frontend/dist
ENV PORT=8080
EXPOSE 8080
CMD ["chat-app"]
