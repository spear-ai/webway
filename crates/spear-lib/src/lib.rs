//! # spear-lib
//!
//! Runtime library for the Spear Data Normalization Gateway.
//!
//! Provides the three components needed to move a raw Middleware WSDL message
//! into Redpanda as a normalized protobuf envelope:
//!
//! ```text
//! WSDL bytes
//!   └─► wsdl::extract_body_payload()   — strip SOAP envelope, return payload XML
//!         └─► serde XML decode          — caller deserializes into generated type
//!               └─► prost::encode()     — caller encodes struct to proto bytes
//!                     └─► ProtoEnvelope::new()  — wrap with metadata
//!                           └─► Publisher::publish()  — send to Redpanda
//! ```
//!
//! ## Modules
//!
//! - [`wsdl`] — SOAP 1.1/1.2 envelope parser
//! - [`envelope`] — [`ProtoEnvelope`]: the protobuf wrapper around every
//!   published message
//! - [`publisher`] — async Redpanda producer backed by `rdkafka`
//!
//! ## Example
//!
//! ```rust,no_run
//! use spear_lib::{ProtoEnvelope, Publisher, wsdl};
//! use prost::Message;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 1. Strip the SOAP wrapper
//! let soap_bytes: &[u8] = b"<soap:Envelope>...</soap:Envelope>";
//! let payload_xml = wsdl::extract_body_payload(soap_bytes)?;
//!
//! // 2. Decode into a generated type (from spear-gen output)
//! // let msg: TrackMessage = quick_xml::de::from_str(&payload_xml)?;
//!
//! // 3. Encode to proto bytes
//! // let proto_bytes = msg.encode_to_vec();
//!
//! // 4. Wrap in an envelope and publish
//! let envelope = ProtoEnvelope::new(1, "middleware", vec![/* proto_bytes */]);
//! let publisher = Publisher::new("localhost:9092")?;
//! publisher.publish("tracks", "TRK-001", &envelope).await?;
//! # Ok(())
//! # }
//! ```

pub mod envelope;
pub mod publisher;
pub mod wsdl;

pub use envelope::ProtoEnvelope;
pub use publisher::Publisher;
