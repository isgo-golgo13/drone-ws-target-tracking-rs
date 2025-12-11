//! TLS WebSocket server with strategy-based packet dispatch.
//!
//! Implements a high-performance async WebSocket server using tokio-tungstenite
//! with rustls for TLS termination.

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use protocol::{Packet, ProtocolApi, StrategyHandler, Urgency};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use svckit::AddrConfig;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

// ============================================================================
// Strategy Implementation
// ============================================================================

/// Server-side strategy handler for incoming packets.
struct ServerStrategyHandler {
    /// Sender for broadcasting drone stream data.
    drone_stream_tx: broadcast::Sender<String>,
}

impl ServerStrategyHandler {
    fn new() -> Self {
        let (drone_stream_tx, _) = broadcast::channel(16);
        Self { drone_stream_tx }
    }
}

#[async_trait]
impl StrategyHandler for ServerStrategyHandler {
    async fn on_urgent_red(&self, packet: &Packet) {
        info!(
            "[SERVER] 🔴 URGENT RED — STREAMING DRONE TARGET DATA: {}",
            packet.payload_string_lossy()
        );

        // Simulate SSE-like drone coordinate stream
        let tx = self.drone_stream_tx.clone();
        tokio::spawn(async move {
            for i in 0..5 {
                let lat = 34.2345 + (i as f64) * 0.0001;
                let lon = 69.1234 + (i as f64) * 0.0002;
                let msg = format!("[DRONE STREAM] lat={:.4}, lon={:.4}", lat, lon);
                info!("{}", msg);
                let _ = tx.send(msg);
                tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
            }
        });
    }

    async fn on_normal(&self, packet: &Packet) {
        info!(
            "[SERVER] 🟢 Normal packet: {}",
            packet.payload_string_lossy()
        );
    }

    async fn on_urgent_yellow(&self, packet: &Packet) {
        info!(
            "[SERVER] 🟡 Yellow priority: {}",
            packet.payload_string_lossy()
        );
    }
}

// ============================================================================
// TLS Configuration
// ============================================================================

/// Load TLS certificates and private key from PEM files.
fn load_tls_config(config: &AddrConfig) -> Result<Arc<rustls::ServerConfig>> {
    // Load certificate chain
    let cert_file = File::open(&config.tls.cert_file)
        .with_context(|| format!("Failed to open cert file: {:?}", config.tls.cert_file))?;
    let mut cert_reader = BufReader::new(cert_file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificates")?;

    if certs.is_empty() {
        anyhow::bail!("No certificates found in {:?}", config.tls.cert_file);
    }

    // Load private key
    let key_file = File::open(&config.tls.key_file)
        .with_context(|| format!("Failed to open key file: {:?}", config.tls.key_file))?;
    let mut key_reader = BufReader::new(key_file);

    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)
        .context("Failed to parse private key")?
        .ok_or_else(|| anyhow::anyhow!("No private key found in {:?}", config.tls.key_file))?;

    // Build server config
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to build TLS server config")?;

    Ok(Arc::new(server_config))
}

// ============================================================================
// WebSocket Session Handler
// ============================================================================

async fn handle_session(
    stream: TcpStream,
    tls_acceptor: TlsAcceptor,
    handler: Arc<ServerStrategyHandler>,
    api: Arc<ProtocolApi>,
) -> Result<()> {
    let peer_addr = stream.peer_addr().ok();
    info!("[SERVER] New connection from {:?}", peer_addr);

    // TLS handshake
    let tls_stream = tls_acceptor
        .accept(stream)
        .await
        .context("TLS handshake failed")?;

    // WebSocket handshake
    let ws_stream = tokio_tungstenite::accept_async(tls_stream)
        .await
        .context("WebSocket handshake failed")?;

    info!("[SERVER] WebSocket session opened for {:?}", peer_addr);

    let (mut ws_sink, mut ws_source) = ws_stream.split();

    // Read loop
    while let Some(msg_result) = ws_source.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                // Create packet and dispatch via strategy
                let packet = api.make_packet(&text, Urgency::Green);
                api.dispatch(&packet, handler.as_ref()).await;

                // Echo back
                if let Err(e) = ws_sink.send(Message::Text(text.clone())).await {
                    warn!("[SERVER] Failed to send response: {}", e);
                    break;
                }
            }
            Ok(Message::Binary(data)) => {
                // Parse as protocol packet if possible
                match Packet::from_bytes(&data) {
                    Ok(packet) => {
                        api.dispatch(&packet, handler.as_ref()).await;
                        // Echo back
                        if let Err(e) = ws_sink.send(Message::Binary(data)).await {
                            warn!("[SERVER] Failed to send response: {}", e);
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("[SERVER] Invalid packet format: {}", e);
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                let _ = ws_sink.send(Message::Pong(data)).await;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(_)) => {
                info!("[SERVER] Client requested close");
                break;
            }
            Ok(Message::Frame(_)) => {}
            Err(e) => {
                error!("[SERVER] WebSocket error: {}", e);
                break;
            }
        }
    }

    info!("[SERVER] WebSocket session closed for {:?}", peer_addr);
    Ok(())
}

// ============================================================================
// Main Server Loop
// ============================================================================

async fn run_server(config: AddrConfig) -> Result<()> {
    // Initialize TLS
    let tls_config = load_tls_config(&config)?;
    let tls_acceptor = TlsAcceptor::from(tls_config);

    // Bind TCP listener
    let listener = TcpListener::bind(config.addr())
        .await
        .with_context(|| format!("Failed to bind to {}", config.addr()))?;

    info!("🚀 Server listening on {}", config.ws_url());

    let handler = Arc::new(ServerStrategyHandler::new());
    let api = Arc::new(ProtocolApi::new());

    // Accept loop
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tls_acceptor = tls_acceptor.clone();
                let handler = Arc::clone(&handler);
                let api = Arc::clone(&api);

                tokio::spawn(async move {
                    if let Err(e) = handle_session(stream, tls_acceptor, handler, api).await {
                        error!("[SERVER] Session error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("[SERVER] Accept error: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ws_server=info".parse().unwrap())
                .add_directive("tokio_tungstenite=info".parse().unwrap()),
        )
        .init();

    let config = AddrConfig::from_env_defaults("0.0.0.0", 8443);

    info!("Starting WebSocket server...");
    info!("  Host: {}", config.host);
    info!("  Port: {}", config.port);
    info!("  Cert: {:?}", config.tls.cert_file);
    info!("  Key:  {:?}", config.tls.key_file);

    run_server(config).await
}
