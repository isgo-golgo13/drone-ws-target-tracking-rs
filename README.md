# Drone WebSockets Target Tracking System (Rust)
TLS WebSocket Server, WebSocket Client and Torpedo Firing Application Protocol using Rust, WebSockets API, Rust Tokio Async API



## Project Structure

```shell
drone-ws-target-tracking-rs/
├── Cargo.toml              # Workspace
├── certificates/           # TLS certs (empty, generate with mkcert)
├── svckit/                 # AddrConfig, TlsConfig
├── protocol/               # Packet, PacketHeader, StrategyHandler trait
├── ws-server/              # TLS WebSocket server
└── ws-client/              # TLS WebSocket client

```


## Building the Project Binariee

```shell
tar -xzf drone-ws-target-tracking-rs.tar.gz
cd drone-ws-target-tracking-rs

# Generate certs
mkdir -p certificates
mkcert -cert-file certificates/server.pem \
       -key-file certificates/server-key.pem \
       localhost 127.0.0.1 ::1

# Build (first run ~30s, incremental ~2s)
cargo build --release

# Terminal 1: Server
cargo run -p ws-server --release

# Terminal 2: Client (interactive mode)
cargo run -p ws-client --release -- -i
```


### Using the root Makefile

```shell
# Local development
make certs
make release
make run-server  # Terminal 1
make run-client  # Terminal 2

# Docker
make up-build
make logs


make help           # Show all targets
make release        # Build release binaries
make certs          # Generate TLS certs with mkcert
make docker-build   # Build both Docker images
make up             # Start services (auto-generates certs)
make down           # Stop services
make logs           # Tail logs
make check          # fmt + clippy + test
make ci-full        # Full CI pipeline
```


