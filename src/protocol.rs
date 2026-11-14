//! ASCII handshake followed by raw little-endian signed 16-bit mono PCM.
//! Handshake: "PKARAOKE/1 <6-digit PIN> <sample-rate>\n"
//! Server response: "OK\n", "ERR AUTH\n", "ERR FORMAT\n", or "ERR BUSY\n".

pub const PORT: u16 = 8765;
pub const HANDSHAKE_MAX: usize = 96;

#[derive(Debug, PartialEq, Eq)]
pub enum HandshakeError {
    Format,
    Authentication,
}

pub fn parse_handshake(line: &str, pin: &str) -> Result<u32, HandshakeError> {
    let mut parts = line.trim_end_matches(['\r', '\n']).split_whitespace();
    if parts.next() != Some("PKARAOKE/1") {
        return Err(HandshakeError::Format);
    }
    let claimed_pin = parts.next().ok_or(HandshakeError::Format)?;
    let rate = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or(HandshakeError::Format)?;
    if parts.next().is_some() || !(8000..=96000).contains(&rate) {
        return Err(HandshakeError::Format);
    }
    if claimed_pin != pin {
        return Err(HandshakeError::Authentication);
    }
    Ok(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_handshake() {
        assert_eq!(
            parse_handshake("PKARAOKE/1 123456 48000\n", "123456"),
            Ok(48000)
        );
    }

    #[test]
    fn denies_wrong_pairing_code() {
        assert_eq!(
            parse_handshake("PKARAOKE/1 000000 48000\n", "123456"),
            Err(HandshakeError::Authentication)
        );
    }

    #[test]
    fn rejects_unbounded_or_malformed_rate() {
        assert_eq!(
            parse_handshake("PKARAOKE/1 123456 0", "123456"),
            Err(HandshakeError::Format)
        );
        assert_eq!(
            parse_handshake("PKARAOKE/1 123456 999999", "123456"),
            Err(HandshakeError::Format)
        );
        assert_eq!(
            parse_handshake("PKARAOKE/1 123456 48000 extraneous", "123456"),
            Err(HandshakeError::Format)
        );
        assert_eq!(
            parse_handshake("GET / HTTP/1.1", "123456"),
            Err(HandshakeError::Format)
        );
    }
}
