//! Encode an AlertMessage to binary, decode it back, and print the result.
//!
//! Run with:
//!   cargo run -p spear-gen --example alert_roundtrip

#[allow(warnings, non_camel_case_types)]
mod types {
    include!("../tests/fixtures/messages.rs");
}

use types::*;

fn main() {
    // ── build a message ───────────────────────────────────────────────────────
    let original = AlertMessage {
        alert_id: "ALT-001".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        severity: AlertSeverity::Critical as i32,
        type_field: AlertType::TrackAlert as i32,
        source: Some(AlertSource {
            system_id: "SYS-ALPHA".to_string(),
            sensor_id: String::new(),
            operator_id: String::new(),
        }),
        description: "Track lost contact".to_string(),
        related_track_ids: vec!["TRK-001".to_string(), "TRK-002".to_string()],
    };

    println!("=== Original ===");
    println!("{original:#?}");

    // ── encode ────────────────────────────────────────────────────────────────
    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);

    println!("\n=== Encoded ({} bytes) ===", buf.len());
    println!(
        "{}",
        buf.iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .chunks(16)
            .map(|chunk| chunk.join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // ── decode ────────────────────────────────────────────────────────────────
    let (decoded, consumed) = AlertMessage::decode_raw(&buf, true).expect("decode failed");

    println!("\n=== Decoded ({consumed} bytes consumed) ===");
    println!("{decoded:#?}");

    // ── verify ────────────────────────────────────────────────────────────────
    assert_eq!(original, decoded, "round-trip mismatch");
    assert_eq!(consumed, buf.len(), "leftover bytes");
    println!("\nRound-trip OK");
}
