//! Round-trip encode/decode tests using the synthetic XSD schemas.
//!
//! The fixture file is generated from schemas/synthetic/ by running:
//!   cargo run -p spear-gen -- \
//!     --input schemas/synthetic \
//!     --out-rust crates/spear-gen/tests/fixtures
//!
//! Regenerate it whenever the emitter or XSD files change.

#[allow(warnings, non_camel_case_types)]
mod types {
    include!("fixtures/messages.rs");
}

use types::*;

// ── TrackMessage ──────────────────────────────────────────────────────────────

#[test]
fn track_message_roundtrip() {
    let original = TrackMessage {
        track_id: "TRK-001".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        category: TrackCategory::Air as i32,
        quality: TrackQuality::Good as i32,
        position: Some(Position {
            latitude: 37.7749,
            longitude: -122.4194,
            altitude: 5000.0,
        }),
        velocity: Some(Velocity {
            speed_knots: 250.0,
            heading_deg: 270.0,
            vertical_rate_fpm: 0.0,
        }),
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size(), "encoded_size mismatch");

    let (decoded, consumed) = TrackMessage::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len(), "not all bytes consumed");
    assert_eq!(original, decoded);
}

#[test]
fn track_message_no_velocity_roundtrip() {
    let original = TrackMessage {
        track_id: "TRK-002".to_string(),
        timestamp: "2024-06-15T12:00:00Z".to_string(),
        category: TrackCategory::Surface as i32,
        quality: TrackQuality::Fair as i32,
        position: Some(Position {
            latitude: 51.5074,
            longitude: -0.1278,
            altitude: 0.0,
        }),
        velocity: None,
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size());

    let (decoded, consumed) = TrackMessage::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len());
    assert_eq!(original, decoded);
}

// ── SensorStatus ──────────────────────────────────────────────────────────────

#[test]
fn sensor_status_roundtrip() {
    let original = SensorStatus {
        sensor_id: "SENSOR-42".to_string(),
        operational: true,
        error_codes: vec![101, 202, 303],
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size());

    let (decoded, consumed) = SensorStatus::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len());
    assert_eq!(original, decoded);
}

#[test]
fn sensor_status_no_errors_roundtrip() {
    let original = SensorStatus {
        sensor_id: "SENSOR-01".to_string(),
        operational: false,
        error_codes: vec![],
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size());

    let (decoded, consumed) = SensorStatus::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len());
    assert_eq!(original, decoded);
}

// ── StatusMessage (xs:extension flattened) ────────────────────────────────────

#[test]
fn status_message_roundtrip() {
    let original = StatusMessage {
        message_id: "MSG-001".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        source_system_id: "SYS-A".to_string(),
        state: SystemState::Operational as i32,
        sensors: vec![
            SensorStatus {
                sensor_id: "S1".to_string(),
                operational: true,
                error_codes: vec![],
            },
            SensorStatus {
                sensor_id: "S2".to_string(),
                operational: false,
                error_codes: vec![404, 500],
            },
        ],
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size());

    let (decoded, consumed) = StatusMessage::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len());
    assert_eq!(original, decoded);
}

// ── AlertMessage (xs:choice + repeated strings) ───────────────────────────────

#[test]
fn alert_message_roundtrip() {
    let original = AlertMessage {
        alert_id: "ALT-001".to_string(),
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        severity: AlertSeverity::Critical as i32,
        type_field: AlertType::TrackAlert as i32,
        source: Some(AlertSource {
            system_id: "SYS-1".to_string(),
            sensor_id: String::new(),
            operator_id: String::new(),
        }),
        description: "Track lost".to_string(),
        related_track_ids: vec!["TRK-001".to_string(), "TRK-002".to_string()],
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size());

    let (decoded, consumed) = AlertMessage::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len());
    assert_eq!(original, decoded);
}

// ── Credentials (Vec<u8> bytes field from primitive alias) ────────────────────

#[test]
fn credentials_bytes_roundtrip() {
    let original = Credentials {
        token: vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0xff],
        label: "ALPHA-1".to_string(),
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size(), "encoded_size mismatch");

    let (decoded, consumed) = Credentials::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len(), "not all bytes consumed");
    assert_eq!(original, decoded);
}

#[test]
fn credentials_empty_token_roundtrip() {
    let original = Credentials {
        token: vec![],
        label: String::new(),
    };

    let mut buf = Vec::new();
    original.encode_raw(&mut buf, true);
    assert_eq!(buf.len(), original.encoded_size());

    let (decoded, consumed) = Credentials::decode_raw(&buf, true).unwrap();
    assert_eq!(consumed, buf.len());
    assert_eq!(original, decoded);
}

// ── endianness ────────────────────────────────────────────────────────────────

#[test]
fn sensor_status_endian_swap_roundtrip() {
    let original = SensorStatus {
        sensor_id: "SWAP".to_string(),
        operational: true,
        error_codes: vec![1, 2, 3],
    };

    // Encode as big-endian (same_endianness=false on a little-endian host)
    let mut buf_be = Vec::new();
    original.encode_raw(&mut buf_be, false);

    // Decode as big-endian
    let (decoded, _) = SensorStatus::decode_raw(&buf_be, false).unwrap();
    assert_eq!(original, decoded);

    // The big-endian and native buffers should differ for the int fields
    let mut buf_ne = Vec::new();
    original.encode_raw(&mut buf_ne, true);
    // String bytes are the same; integer bytes differ
    assert_ne!(buf_be, buf_ne);
}
