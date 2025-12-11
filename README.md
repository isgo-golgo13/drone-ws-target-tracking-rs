# Drone WebSockets Target Tracking System (Rust)
TLS WebSocket Server, WebSocket Client and Torpedo Firing Application Protocol using Rust, WebSockets API, Rust Tokio Async API




## Building the Project Binariee

```shell
tar -xzf drone-ws-target-tracking-cxx.tar.gz
cd drone-ws-target-tracking-cxx

# Generate certs first
mkdir -p certificates
mkcert -install
mkcert -cert-file certificates/server.pem -key-file certificates/server-key.pem localhost 127.0.0.1 ::1

# Configure and build

```

