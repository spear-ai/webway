//! spear-gateway — Redpanda consumer for normalized SPEAR messages.
//!
//! ## Classified-side setup
//!
//! 1. Run `spear-gen` against your real XSDs to get a generated type file:
//!    ```bash
//!    spear-gen --input /workspace/xsds \
//!              --out-proto /workspace/types.proto \
//!              --out-rust   /workspace/types.rs
//!    ```
//!
//! 2. Copy the generated file into this crate:
//!    ```bash
//!    cp /workspace/types.rs /spear/crates/spear-gateway/src/types.rs
//!    ```
//!
//! 3. Uncomment the `include!` line below in `handle_message` and add your
//!    `decode_raw` call.
//!
//! 4. Rebuild offline:
//!    ```bash
//!    cargo build --offline --release -p spear-gateway
//!    ```

use anyhow::{Context, Result};
use clap::Parser;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::ClientConfig;
use spear_lib::ProtoEnvelope;

// ─── generated types ─────────────────────────────────────────────────────────
// After running spear-gen on your XSDs, copy the output here and uncomment:
//
//   include!("types.rs");
//
// Then add your decode_raw call inside `handle_message` below.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "spear-gateway", about = "Consume SPEAR messages from Redpanda")]
struct Args {
    /// Redpanda broker list (comma-separated)
    #[arg(long, env = "REDPANDA_BROKERS", default_value = "localhost:9092")]
    brokers: String,

    /// Topic to consume
    #[arg(long, env = "SPEAR_TOPIC", default_value = "spear.messages")]
    topic: String,

    /// Consumer group ID
    #[arg(long, env = "SPEAR_GROUP", default_value = "spear-gateway")]
    group: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    eprintln!("[spear-gateway] brokers={}", args.brokers);
    eprintln!("[spear-gateway] topic={}", args.topic);
    eprintln!("[spear-gateway] group={}", args.group);

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &args.brokers)
        .set("group.id", &args.group)
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "true")
        .create()
        .context("creating Redpanda consumer")?;

    consumer
        .subscribe(&[&args.topic])
        .context("subscribing to topic")?;

    eprintln!("[spear-gateway] subscribed, waiting for messages...");

    loop {
        match consumer.recv().await {
            Ok(msg) => {
                let payload = match msg.payload() {
                    Some(p) => p,
                    None => {
                        eprintln!("[spear-gateway] received empty message, skipping");
                        continue;
                    }
                };
                if let Err(e) = handle_message(payload) {
                    eprintln!("[spear-gateway] error handling message: {e}");
                }
            }
            Err(e) => {
                eprintln!("[spear-gateway] kafka error: {e}");
            }
        }
    }
}

fn handle_message(raw: &[u8]) -> Result<()> {
    let envelope = ProtoEnvelope::decode_from_bytes(raw).context("decoding ProtoEnvelope")?;

    eprintln!(
        "[msg] schema_v={} adapter={} ts_ms={} payload_bytes={}",
        envelope.schema_version,
        envelope.source_adapter_id,
        envelope.timestamp_ms,
        envelope.payload.len(),
    );

    // ── decode generated types ────────────────────────────────────────────────
    // After adding your generated types (see module comment at top of file),
    // replace this block with the appropriate decode_raw call, e.g.:
    //
    //   let (msg, _sz) = TrackMessage::decode_raw(&envelope.payload, true)?;
    //   println!("{msg:#?}");
    //
    // Use `envelope.schema_version` to dispatch to the right type if you
    // publish multiple message types on the same topic.
    // ─────────────────────────────────────────────────────────────────────────

    if !envelope.payload.is_empty() {
        eprintln!("[msg] payload (hex): {}", hex_dump(&envelope.payload, 64));
    }

    Ok(())
}

/// Print up to `limit` bytes as hex.
fn hex_dump(bytes: &[u8], limit: usize) -> String {
    let truncated = bytes.len() > limit;
    let slice = &bytes[..bytes.len().min(limit)];
    let hex: String = slice
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    if truncated {
        format!("{hex} ... ({} bytes total)", bytes.len())
    } else {
        hex
    }
}
