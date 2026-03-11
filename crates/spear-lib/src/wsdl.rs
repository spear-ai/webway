//! SOAP envelope parser.
//!
//! Middleware messages arrive wrapped in a SOAP 1.1 or 1.2 envelope:
//!
//! ```xml
//! <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
//!   <soap:Header> ... </soap:Header>   <!-- optional, ignored -->
//!   <soap:Body>
//!     <TrackMessage>                   <!-- this is what we want -->
//!       <TrackId>TRK-001</TrackId>
//!       ...
//!     </TrackMessage>
//!   </soap:Body>
//! </soap:Envelope>
//! ```
//!
//! [`extract_body_payload`] strips the envelope and returns the inner payload
//! as a UTF-8 XML string ready for deserialization with
//! `quick_xml::de::from_str` into a generated type.
//!
//! Both `soap:` (SOAP 1.1) and `env:` (SOAP 1.2) namespace prefixes are
//! recognized. `xmlns` declarations are stripped from the extracted payload
//! since the generated serde deserializers work on local element names only.

use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

const SOAP_ENV_NS: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const SOAP_ENV_NS_12: &str = "http://www.w3.org/2003/05/soap-envelope";

/// Extracts the first child element of `<soap:Body>` from a SOAP 1.1 or 1.2
/// envelope and returns it as a UTF-8 XML string.
///
/// The gateway calls this before deserializing the payload into a generated
/// Rust type with `quick_xml::de::from_str`.
pub fn extract_body_payload(soap_bytes: &[u8]) -> Result<String> {
    let soap_str = std::str::from_utf8(soap_bytes).context("SOAP message is not valid UTF-8")?;

    let mut reader = Reader::from_str(soap_str);
    reader.trim_text(true);

    let mut in_body = false;
    let mut depth_in_body: u32 = 0;
    let mut payload_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let name_owned = e.name().as_ref().to_vec();
                let local = local_name(&name_owned);
                let ns = resolve_ns(&reader, &name_owned);

                if !in_body {
                    if local == "Body" && is_soap_ns(&ns) {
                        in_body = true;
                        depth_in_body = 0;
                        continue;
                    }
                } else {
                    depth_in_body += 1;
                    // Capture everything inside Body verbatim.
                    append_start_tag(e, soap_str, &mut payload_buf);
                }
            }
            Ok(Event::End(ref e)) => {
                let name_owned = e.name().as_ref().to_vec();
                let local = local_name(&name_owned);
                let ns = resolve_ns(&reader, &name_owned);

                if in_body {
                    if depth_in_body == 0 {
                        // Closing </soap:Body>
                        break;
                    }
                    depth_in_body -= 1;
                    payload_buf.push_str("</");
                    payload_buf.push_str(local);
                    payload_buf.push('>');
                } else if local == "Body" && is_soap_ns(&ns) {
                    // Empty body
                    break;
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_body && depth_in_body > 0 {
                    payload_buf.push_str(&e.unescape().unwrap_or_default());
                }
            }
            Ok(Event::Empty(ref e)) => {
                if in_body && depth_in_body > 0 {
                    append_empty_tag(e, soap_str, &mut payload_buf);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!(
                "XML parse error at position {}: {e}",
                reader.buffer_position()
            ),
            _ => {}
        }
    }

    if payload_buf.is_empty() {
        bail!("no payload found in SOAP Body");
    }

    Ok(payload_buf)
}

/// Returns the local name (stripping any namespace prefix).
fn local_name(raw: &[u8]) -> &str {
    let s = std::str::from_utf8(raw).unwrap_or("");
    s.rfind(':').map(|i| &s[i + 1..]).unwrap_or(s)
}

fn is_soap_ns(ns: &str) -> bool {
    ns == SOAP_ENV_NS || ns == SOAP_ENV_NS_12 || ns.is_empty()
}

/// quick-xml doesn't give us resolved namespace strings easily without the
/// NsReader. We do a simple prefix-based check for the common soap prefixes.
fn resolve_ns(reader: &Reader<&[u8]>, raw: &[u8]) -> String {
    let _ = reader;
    let s = std::str::from_utf8(raw).unwrap_or("");
    if let Some(colon) = s.find(':') {
        let prefix = &s[..colon];
        match prefix {
            "soap" | "soapenv" | "SOAP-ENV" | "env" => SOAP_ENV_NS.to_owned(),
            _ => String::new(),
        }
    } else {
        String::new()
    }
}

fn append_start_tag(e: &quick_xml::events::BytesStart, _src: &str, buf: &mut String) {
    buf.push('<');
    buf.push_str(local_name(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
        // Drop xmlns declarations — the payload doesn't need them for our
        // internal deserialization purposes.
        if key.starts_with("xmlns") {
            continue;
        }
        let val = attr.unescape_value().unwrap_or_default();
        buf.push(' ');
        buf.push_str(key);
        buf.push_str("=\"");
        buf.push_str(&val);
        buf.push('"');
    }
    buf.push('>');
}

fn append_empty_tag(e: &quick_xml::events::BytesStart, src: &str, buf: &mut String) {
    append_start_tag(e, src, buf);
    // Replace trailing `>` with `/>`
    if buf.ends_with('>') {
        buf.pop();
        buf.push_str("/>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_body_payload() {
        let soap = r#"<?xml version="1.0"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <TrackMessage>
      <TrackId>TRK-001</TrackId>
      <Latitude>38.9</Latitude>
    </TrackMessage>
  </soap:Body>
</soap:Envelope>"#;

        let payload = extract_body_payload(soap.as_bytes()).unwrap();
        assert!(payload.contains("TrackMessage"));
        assert!(payload.contains("TRK-001"));
    }

    #[test]
    fn rejects_missing_body() {
        let xml = b"<root><child/></root>";
        assert!(extract_body_payload(xml).is_err());
    }
}
