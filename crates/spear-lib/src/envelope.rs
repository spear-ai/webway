//! [`ProtoEnvelope`] — the protobuf wrapper around every message published
//! to Redpanda.
//!
//! Every adapter produces a `ProtoEnvelope` regardless of the source protocol.
//! This gives consumers a consistent outer structure: they always decode a
//! `ProtoEnvelope` first, then use `schema_version` to determine which
//! generated type to decode `payload` into.
//!
//! ## Schema versioning
//!
//! `schema_version` starts at `1` and is bumped when the generated schema
//! changes in a breaking way. A major bump produces a parallel Redpanda topic
//! (e.g. `tracks.v1`, `tracks.v2`) so existing consumers are not disrupted.
//!
//! ## Wire format
//!
//! `ProtoEnvelope` is itself protobuf-encoded. The `payload` field contains
//! the prost-encoded inner message (e.g. a `TrackMessage`).

use anyhow::Result;
use prost::Message;

/// Wraps every normalized message published to Redpanda.
///
/// Consumers read the schema_version to know which generated type to
/// deserialize the payload bytes into.
#[derive(Clone, PartialEq, prost::Message)]
pub struct ProtoEnvelope {
    /// Incremented when the generated schema changes in a breaking way.
    /// A major bump produces a parallel Redpanda topic (v1, v2, ...).
    #[prost(uint32, tag = "1")]
    pub schema_version: u32,

    /// Unix epoch milliseconds at time of normalization.
    #[prost(int64, tag = "2")]
    pub timestamp_ms: i64,

    /// Identifies which adapter produced this message (e.g. "middleware").
    #[prost(string, tag = "3")]
    pub source_adapter_id: String,

    /// prost-encoded payload (the generated message type).
    #[prost(bytes = "vec", tag = "4")]
    pub payload: Vec<u8>,
}

impl ProtoEnvelope {
    pub fn new(
        schema_version: u32,
        source_adapter_id: impl Into<String>,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            schema_version,
            timestamp_ms: now_ms(),
            source_adapter_id: source_adapter_id.into(),
            payload,
        }
    }

    /// Serialize the envelope to bytes for publishing to Redpanda.
    pub fn encode_to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buf)?;
        Ok(buf)
    }

    /// Deserialize an envelope from Redpanda bytes.
    pub fn decode_from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(Self::decode(bytes)?)
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let env = ProtoEnvelope::new(1, "middleware", b"fake-payload".to_vec());
        let bytes = env.encode_to_bytes().unwrap();
        let decoded = ProtoEnvelope::decode_from_bytes(&bytes).unwrap();
        assert_eq!(env.schema_version, decoded.schema_version);
        assert_eq!(env.source_adapter_id, decoded.source_adapter_id);
        assert_eq!(env.payload, decoded.payload);
    }
}
