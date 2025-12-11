//! Binary protocol with bitfield headers and strategy-based dispatch.
//!
//! Implements a clean Strategy pattern for handling different packet urgency levels,
//! enabling polymorphic behavior for drone target tracking scenarios.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Protocol version constant.
pub const PROTOCOL_VERSION: u8 = 1;

/// Packet type for standard messages.
pub const PACKET_TYPE_MESSAGE: u8 = 1;

/// Urgency levels for packet prioritization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Urgency {
    /// Normal priority - routine operations
    Green = 0,
    /// Elevated priority - time-sensitive data
    Yellow = 1,
    /// Critical priority - immediate action required (drone target lock)
    Red = 2,
}

impl From<u8> for Urgency {
    fn from(value: u8) -> Self {
        match value {
            0 => Urgency::Green,
            1 => Urgency::Yellow,
            2 => Urgency::Red,
            _ => Urgency::Green,
        }
    }
}

impl Urgency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Urgency::Green => "GREEN",
            Urgency::Yellow => "YELLOW",
            Urgency::Red => "RED",
        }
    }
}

/// Packed header for wire protocol.
///
/// Layout (6 bytes total):
/// - version: 4 bits
/// - type: 4 bits
/// - urgent: 2 bits
/// - reserved: 6 bits
/// - length: 32 bits
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PacketHeader {
    pub version: u8,
    pub packet_type: u8,
    pub urgency: Urgency,
    pub length: u32,
}

impl PacketHeader {
    /// Create a new header with the given urgency and payload length.
    pub fn new(urgency: Urgency, length: u32) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            packet_type: PACKET_TYPE_MESSAGE,
            urgency,
            length,
        }
    }

    /// Serialize header to wire format (6 bytes).
    pub fn to_bytes(&self) -> [u8; 6] {
        let byte0 = (self.version & 0x0F) | ((self.packet_type & 0x0F) << 4);
        let byte1 = (self.urgency as u8) & 0x03; // 2 bits urgency, 6 bits reserved (zeros)
        let len_bytes = self.length.to_be_bytes();

        [byte0, byte1, len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]
    }

    /// Deserialize header from wire format.
    pub fn from_bytes(bytes: &[u8; 6]) -> Self {
        let version = bytes[0] & 0x0F;
        let packet_type = (bytes[0] >> 4) & 0x0F;
        let urgency = Urgency::from(bytes[1] & 0x03);
        let length = u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);

        Self {
            version,
            packet_type,
            urgency,
            length,
        }
    }
}

/// Complete packet with header and payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl Packet {
    /// Create a new packet from a message string and urgency level.
    pub fn new(message: impl AsRef<str>, urgency: Urgency) -> Self {
        let payload = message.as_ref().as_bytes().to_vec();
        let header = PacketHeader::new(urgency, payload.len() as u32);
        Self { header, payload }
    }

    /// Create a GREEN urgency packet (convenience method).
    pub fn green(message: impl AsRef<str>) -> Self {
        Self::new(message, Urgency::Green)
    }

    /// Create a YELLOW urgency packet (convenience method).
    pub fn yellow(message: impl AsRef<str>) -> Self {
        Self::new(message, Urgency::Yellow)
    }

    /// Create a RED urgency packet (convenience method).
    pub fn red(message: impl AsRef<str>) -> Self {
        Self::new(message, Urgency::Red)
    }

    /// Get payload as UTF-8 string.
    pub fn payload_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.payload)
    }

    /// Get payload as owned String, lossy conversion.
    pub fn payload_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.payload).into_owned()
    }

    /// Serialize entire packet to wire format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(6 + self.payload.len());
        bytes.extend_from_slice(&self.header.to_bytes());
        bytes.extend_from_slice(&self.payload);
        bytes
    }

    /// Deserialize packet from wire format.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() < 6 {
            return Err(ProtocolError::InsufficientData {
                expected: 6,
                actual: bytes.len(),
            });
        }

        let header_bytes: [u8; 6] = bytes[0..6].try_into().unwrap();
        let header = PacketHeader::from_bytes(&header_bytes);

        let expected_len = 6 + header.length as usize;
        if bytes.len() < expected_len {
            return Err(ProtocolError::InsufficientData {
                expected: expected_len,
                actual: bytes.len(),
            });
        }

        let payload = bytes[6..expected_len].to_vec();

        Ok(Self { header, payload })
    }

    /// Convert to JSON representation.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "version": self.header.version,
            "type": self.header.packet_type,
            "urgency": self.header.urgency.as_str(),
            "length": self.header.length,
            "payload": self.payload_string_lossy()
        })
    }
}

/// Protocol errors.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("Insufficient data: expected {expected} bytes, got {actual}")]
    InsufficientData { expected: usize, actual: usize },

    #[error("Invalid packet format: {0}")]
    InvalidFormat(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

// ============================================================================
// Strategy Pattern
// ============================================================================

/// Strategy handler trait for packet dispatch.
///
/// Implementors define behavior for different urgency levels,
/// enabling clean separation of concerns for packet processing.
///
/// # Example
///
/// ```rust,ignore
/// struct DroneController;
///
/// #[async_trait]
/// impl StrategyHandler for DroneController {
///     async fn on_urgent_red(&self, packet: &Packet) {
///         // Engage torpedo lock!
///     }
///
///     async fn on_normal(&self, packet: &Packet) {
///         // Routine telemetry processing
///     }
/// }
/// ```
#[async_trait]
pub trait StrategyHandler: Send + Sync {
    /// Handle RED urgency packets - critical priority requiring immediate action.
    async fn on_urgent_red(&self, packet: &Packet);

    /// Handle YELLOW urgency packets - elevated priority.
    async fn on_urgent_yellow(&self, packet: &Packet) {
        // Default: treat as normal
        self.on_normal(packet).await;
    }

    /// Handle GREEN urgency packets - normal priority.
    async fn on_normal(&self, packet: &Packet);
}

/// Protocol API for packet creation and dispatch.
#[derive(Debug, Default)]
pub struct ProtocolApi;

impl ProtocolApi {
    pub fn new() -> Self {
        Self
    }

    /// Create a packet from message and urgency.
    pub fn make_packet(&self, message: impl AsRef<str>, urgency: Urgency) -> Packet {
        Packet::new(message, urgency)
    }

    /// Dispatch packet to appropriate strategy handler method.
    pub async fn dispatch<H: StrategyHandler>(&self, packet: &Packet, handler: &H) {
        match packet.header.urgency {
            Urgency::Red => handler.on_urgent_red(packet).await,
            Urgency::Yellow => handler.on_urgent_yellow(packet).await,
            Urgency::Green => handler.on_normal(packet).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_roundtrip() {
        let original = Packet::red("TORPEDO LOCKED ON TARGET");
        let bytes = original.to_bytes();
        let decoded = Packet::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.header.version, PROTOCOL_VERSION);
        assert_eq!(decoded.header.urgency, Urgency::Red);
        assert_eq!(decoded.payload_str().unwrap(), "TORPEDO LOCKED ON TARGET");
    }

    #[test]
    fn test_header_bitpacking() {
        let header = PacketHeader::new(Urgency::Yellow, 1024);
        let bytes = header.to_bytes();
        let decoded = PacketHeader::from_bytes(&bytes);

        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.urgency, Urgency::Yellow);
        assert_eq!(decoded.length, 1024);
    }
}
