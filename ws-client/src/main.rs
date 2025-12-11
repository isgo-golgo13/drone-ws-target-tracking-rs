//! TLS WebSocket client with strategy-based packet dispatch.
//!
//! Implements a high-performance async WebSocket client using tokio-tungstenite
//! with rustls for TLS validation.

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use protocol::{Packet, ProtocolApi, StrategyHandler, Urgency};
use rustls::pki_types::{CertificateDer, ServerName};
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use svckit::AddrConfig;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

// ============================================================================
// Strategy Implementation
// ============================================================================

/// Client-side strategy handler for incoming packets.
struct ClientStrategyHandler;

#[async_trait]
impl StrategyHandler for ClientStrategyHandler {
    async fn on_urgent_red(&self, packet: &Packet) {
        info!(
            "[CLIENT] 🔴 RED ALERT: Drone target received: {}",
            packet.payload_string_lossy()
        );
    }

    async fn on_normal(&self, packet: &Packet) {
        info!(
            "[CLIENT] 🟢 Normal response from server: {}",
            packet.payload_string_lossy()
        );
    }

    async fn on_urgent_yellow(&self, packet: &Packet) {
        info!(
            "[CLIENT] 🟡 Yellow priority response: {}",
            packet.payload_string_lossy()
        );
    }
}

// ============================================================================
// TLS Configuration
// ============================================================================

/// Load TLS client configuration with custom CA certificate.
fn load_tls_config(config: &AddrConfig) -> Result<Arc<rustls::ClientConfig>> {
    // Load CA certificate for server verification
    let ca_file = File::open(&config.tls.ca_file)
        .with_context(|| format!("Failed to open CA file: {:?}", config.tls.ca_file))?;
    let mut ca_reader = BufReader::new(ca_file);

    let ca_certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut ca_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse CA certificates")?;

    // Build root cert store
    let mut root_store = rustls::RootCertStore::empty();
    for cert in ca_certs {
        root_store.add(cert).context("Failed to add CA certificate")?;
    }

    // Build client config
    let client_config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(client_config))
}

// ============================================================================
// WebSocket Client Session
// ============================================================================

async fn run_client_session(
    config: AddrConfig,
    initial_message: &str,
) -> Result<()> {
    let tls_config = load_tls_config(&config)?;
    let tls_connector = TlsConnector::from(tls_config);

    // Connect TCP
    let tcp_stream = TcpStream::connect(config.addr())
        .await
        .with_context(|| format!("Failed to connect to {}", config.addr()))?;

    info!("[CLIENT] TCP connected to {}", config.addr());

    // TLS handshake
    let server_name: ServerName<'static> = config
        .host
        .clone()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid server name: {}", config.host))?;

    let tls_stream = tls_connector
        .connect(server_name, tcp_stream)
        .await
        .context("TLS handshake failed")?;

    info!("[CLIENT] TLS handshake complete");

    // WebSocket handshake over TLS stream
    let ws_url = config.ws_url();
    let (ws_stream, _response) = tokio_tungstenite::client_async(&ws_url, tls_stream)
        .await
        .context("WebSocket handshake failed")?;

    info!("[CLIENT] ✅ Connected to {}", ws_url);

    let (mut ws_sink, mut ws_source) = ws_stream.split();

    let handler = ClientStrategyHandler;
    let api = ProtocolApi::new();

    // Send initial message
    ws_sink
        .send(Message::Text(initial_message.to_string()))
        .await
        .context("Failed to send initial message")?;

    info!("[CLIENT] Sent initial message: {}", initial_message);

    // Read loop
    while let Some(msg_result) = ws_source.next().await {
        match msg_result {
            Ok(Message::Text(text)) => {
                let packet = api.make_packet(&text, Urgency::Green);
                api.dispatch(&packet, &handler).await;
            }
            Ok(Message::Binary(data)) => {
                match Packet::from_bytes(&data) {
                    Ok(packet) => {
                        api.dispatch(&packet, &handler).await;
                    }
                    Err(e) => {
                        warn!("[CLIENT] Invalid packet format: {}", e);
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                let _ = ws_sink.send(Message::Pong(data)).await;
            }
            Ok(Message::Pong(_)) => {}
            Ok(Message::Close(frame)) => {
                info!("[CLIENT] Server closed connection: {:?}", frame);
                break;
            }
            Ok(Message::Frame(_)) => {}
            Err(e) => {
                error!("[CLIENT] WebSocket error: {}", e);
                break;
            }
        }
    }

    info!("[CLIENT] Connection closed");
    Ok(())
}

// ============================================================================
// Interactive Client Mode
// ============================================================================

async fn run_interactive_client(config: AddrConfig) -> Result<()> {
    let tls_config = load_tls_config(&config)?;
    let tls_connector = TlsConnector::from(tls_config);

    // Connect TCP
    let tcp_stream = TcpStream::connect(config.addr())
        .await
        .with_context(|| format!("Failed to connect to {}", config.addr()))?;

    // TLS handshake
    let server_name: ServerName<'static> = config
        .host
        .clone()
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid server name: {}", config.host))?;

    let tls_stream = tls_connector
        .connect(server_name, tcp_stream)
        .await
        .context("TLS handshake failed")?;

    // WebSocket handshake
    let ws_url = config.ws_url();
    let (ws_stream, _) = tokio_tungstenite::client_async(&ws_url, tls_stream)
        .await
        .context("WebSocket handshake failed")?;

    info!("[CLIENT] ✅ Connected to {}", ws_url);
    info!("[CLIENT] Type messages to send. Commands:");
    info!("  !red <msg>    - Send RED urgency packet");
    info!("  !yellow <msg> - Send YELLOW urgency packet");
    info!("  !quit         - Exit");

    let (mut ws_sink, mut ws_source) = ws_stream.split();

    let handler = ClientStrategyHandler;
    let api = ProtocolApi::new();

    // Spawn reader task
    let reader_handle = tokio::spawn(async move {
        while let Some(msg_result) = ws_source.next().await {
            match msg_result {
                Ok(Message::Text(text)) => {
                    let packet = api.make_packet(&text, Urgency::Green);
                    api.dispatch(&packet, &handler).await;
                }
                Ok(Message::Binary(data)) => {
                    if let Ok(packet) = Packet::from_bytes(&data) {
                        api.dispatch(&packet, &handler).await;
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    error!("[CLIENT] Read error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Stdin reader loop
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        use tokio::io::AsyncBufReadExt;
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "!quit" {
                    info!("[CLIENT] Exiting...");
                    let _ = ws_sink.send(Message::Close(None)).await;
                    break;
                }

                let (urgency, msg) = if let Some(rest) = trimmed.strip_prefix("!red ") {
                    (Urgency::Red, rest)
                } else if let Some(rest) = trimmed.strip_prefix("!yellow ") {
                    (Urgency::Yellow, rest)
                } else {
                    (Urgency::Green, trimmed)
                };

                let packet = Packet::new(msg, urgency);
                info!(
                    "[CLIENT] Sending {} packet: {}",
                    urgency.as_str(),
                    msg
                );

                // Send as binary protocol packet
                if let Err(e) = ws_sink.send(Message::Binary(packet.to_bytes().into())).await {
                    error!("[CLIENT] Send error: {}", e);
                    break;
                }
            }
            Err(e) => {
                error!("[CLIENT] Stdin error: {}", e);
                break;
            }
        }
    }

    reader_handle.abort();
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ws_client=info".parse().unwrap())
                .add_directive("tokio_tungstenite=info".parse().unwrap()),
        )
        .init();

    let config = AddrConfig::from_env_defaults("localhost", 8443);

    info!("Starting WebSocket client...");
    info!("  Host: {}", config.host);
    info!("  Port: {}", config.port);
    info!("  CA:   {:?}", config.tls.ca_file);

    // Check for --interactive flag
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--interactive" || a == "-i") {
        run_interactive_client(config).await
    } else {
        run_client_session(config, "HELLO FROM CLIENT").await
    }
}
