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

