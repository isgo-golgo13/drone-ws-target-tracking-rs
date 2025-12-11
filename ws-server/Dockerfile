# =============================================================================
# ws-server Dockerfile - Rootless Multi-Stage Build
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Chef - Dependency caching layer
# -----------------------------------------------------------------------------
FROM rust:1.83-slim-bookworm AS chef

RUN cargo install cargo-chef --locked
WORKDIR /app

# -----------------------------------------------------------------------------
# Stage 2: Planner - Analyze dependencies
# -----------------------------------------------------------------------------
FROM chef AS planner

# Copy workspace manifests
COPY Cargo.toml ./
COPY svckit/Cargo.toml svckit/
COPY protocol/Cargo.toml protocol/
COPY ws-server/Cargo.toml ws-server/
COPY ws-client/Cargo.toml ws-client/

# Create dummy source files to satisfy cargo
RUN mkdir -p svckit/src protocol/src ws-server/src ws-client/src && \
    echo "fn main() {}" > ws-server/src/main.rs && \
    echo "fn main() {}" > ws-client/src/main.rs && \
    echo "" > svckit/src/lib.rs && \
    echo "" > protocol/src/lib.rs

RUN cargo chef prepare --recipe-path recipe.json

# -----------------------------------------------------------------------------
# Stage 3: Builder - Compile with cached dependencies
# -----------------------------------------------------------------------------
FROM chef AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy recipe and build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy actual source code
COPY Cargo.toml ./
COPY svckit svckit
COPY protocol protocol
COPY ws-server ws-server
COPY ws-client ws-client

# Build the server binary
RUN cargo build --release -p ws-server && \
    strip target/release/ws-server

# -----------------------------------------------------------------------------
# Stage 4: Runtime - Minimal rootless image
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    netcat-openbsd \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user
RUN groupadd --gid 1000 appgroup && \
    useradd --uid 1000 --gid appgroup --shell /bin/bash --create-home appuser

# Create directories
RUN mkdir -p /app /certificates && \
    chown -R appuser:appgroup /app /certificates

# Switch to non-root user
USER appuser
WORKDIR /app

# Copy binary from builder
COPY --from=builder --chown=appuser:appgroup /app/target/release/ws-server .

# Environment configuration
ENV RUST_LOG=info
ENV CERT_PATH=/certificates

# Mount point for TLS certificates
VOLUME ["/certificates"]

# Expose WebSocket port
EXPOSE 8443

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD nc -z localhost 8443 || exit 1

# Run as non-root
ENTRYPOINT ["./ws-server"]
