//! Async Redpanda publisher backed by `rdkafka`.
//!
//! [`Publisher`] wraps a `FutureProducer` and exposes a single
//! [`Publisher::publish`] method that encodes a [`ProtoEnvelope`] and sends
//! it to a Redpanda topic.
//!
//! ## Partition key
//!
//! Pass a stable identifier as `partition_key` (e.g. track ID, system ID)
//! so that all messages for the same entity land on the same partition and
//! consumers see them in order.
//!
//! ## Cloning
//!
//! [`Publisher`] is cheaply cloneable — `FutureProducer` is `Arc`-backed
//! internally. One publisher instance can be shared across multiple adapter
//! tasks without additional synchronization.

use anyhow::{Context, Result};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use std::time::Duration;

use crate::envelope::ProtoEnvelope;

/// Publishes `ProtoEnvelope` messages to a Redpanda topic.
///
/// One publisher instance is shared per adapter. It is cheaply cloneable
/// because `FutureProducer` is `Arc`-backed internally.
#[derive(Clone)]
pub struct Publisher {
    producer: FutureProducer,
}

impl Publisher {
    /// Create a new publisher connected to the given broker list.
    ///
    /// `brokers` is a comma-separated list, e.g. `"localhost:9092"` or
    /// `"broker1:9092,broker2:9092"`.
    pub fn new(brokers: &str) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            // Deliver messages in order within a partition.
            .set("enable.idempotence", "true")
            .create()
            .context("failed to create Redpanda producer")?;

        Ok(Self { producer })
    }

    /// Encode and publish an envelope to `topic`.
    ///
    /// `partition_key` determines which Redpanda partition receives the
    /// message — use a stable identifier like track ID or system ID so
    /// that messages for the same entity land on the same partition.
    pub async fn publish(
        &self,
        topic: &str,
        partition_key: &str,
        envelope: &ProtoEnvelope,
    ) -> Result<()> {
        let bytes = envelope
            .encode_to_bytes()
            .context("failed to encode ProtoEnvelope")?;

        let record = FutureRecord::to(topic)
            .key(partition_key)
            .payload(bytes.as_slice());

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(err, _msg)| anyhow::anyhow!("Redpanda produce error: {err}"))?;

        Ok(())
    }
}
