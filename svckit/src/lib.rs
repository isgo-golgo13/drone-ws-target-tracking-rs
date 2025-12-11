//! Service configuration toolkit for address and TLS configuration.
//!
//! Provides the `AddrConfig` parameter object following the Open-Closed Principle,
//! allowing future parameter extension without disrupting existing API consumers.

use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;

/// TLS certificate paths configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert_file: PathBuf,
    pub key_file: PathBuf,
    pub ca_file: PathBuf,
}

impl TlsConfig {
    /// Create TLS config from environment-derived paths.
    ///
    /// Uses `CERT_PATH` environment variable if set, otherwise falls back to `./certificates`.
    pub fn from_env() -> Self {
        let base = env::var("CERT_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("certificates"));

        Self {
            cert_file: base.join("server.pem"),
            key_file: base.join("server-key.pem"),
            ca_file: base.join("server.pem"),
        }
    }

    /// Create TLS config from explicit paths.
    pub fn new(cert_file: PathBuf, key_file: PathBuf, ca_file: PathBuf) -> Self {
        Self {
            cert_file,
            key_file,
            ca_file,
        }
    }
}

/// Protocol hint for connection type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolHint {
    #[default]
    Wss,
    Ws,
}

impl ProtocolHint {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProtocolHint::Wss => "wss",
            ProtocolHint::Ws => "ws",
        }
    }
}

/// Address and TLS configuration for WebSocket services.
///
/// This parameter object provides OCP (Open-Closed Principle) extension,
/// allowing future parameter additions without disrupting existing code.
///
/// # Example
///
/// ```rust
/// use svckit::AddrConfig;
///
/// let config = AddrConfig::from_env_defaults("localhost", 8443);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddrConfig {
    pub host: String,
    pub port: u16,
    pub tls: TlsConfig,
    pub endpoint: String,
    pub protocol_hint: ProtocolHint,
    pub use_tls: bool,
}

impl AddrConfig {
    /// Create a new configuration with explicit values.
    pub fn new(host: impl Into<String>, port: u16, tls: TlsConfig) -> Self {
        Self {
            host: host.into(),
            port,
            tls,
            endpoint: "/".to_string(),
            protocol_hint: ProtocolHint::Wss,
            use_tls: true,
        }
    }

    /// Create configuration from environment defaults.
    ///
    /// Uses `CERT_PATH` environment variable for certificate paths,
    /// falling back to `./certificates` if not set.
    pub fn from_env_defaults(host: impl Into<String>, port: u16) -> Self {
        Self::new(host, port, TlsConfig::from_env())
    }

    /// Returns the full WebSocket URL.
    pub fn ws_url(&self) -> String {
        format!(
            "{}://{}:{}{}",
            self.protocol_hint.as_str(),
            self.host,
            self.port,
            self.endpoint
        )
    }

    /// Returns the host:port address string.
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Builder method to set endpoint.
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Builder method to disable TLS.
    pub fn without_tls(mut self) -> Self {
        self.use_tls = false;
        self.protocol_hint = ProtocolHint::Ws;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_addr_config_defaults() {
        let cfg = AddrConfig::from_env_defaults("localhost", 8443);
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 8443);
        assert!(cfg.use_tls);
        assert_eq!(cfg.ws_url(), "wss://localhost:8443/");
    }

    #[test]
    fn test_addr_config_without_tls() {
        let cfg = AddrConfig::from_env_defaults("localhost", 8080).without_tls();
        assert!(!cfg.use_tls);
        assert_eq!(cfg.ws_url(), "ws://localhost:8080/");
    }
}
