//! Bytes → `String` for IGC files.
//!
//! IGC is *spec'd* as ASCII, but recorders in the field happily write
//! 8-bit codepages in the metadata H-records (pilot name, glider type,
//! pilot's club, etc.). The B-records — the actual track points —
//! are always pure ASCII (digits, hemisphere letters), so any encoding
//! issue lives strictly in the metadata.
//!
//! Policy: try UTF-8 first; if that fails, fall back to **Windows-1251**
//! (Cyrillic). cp1251 is the second most common IGC encoding after
//! UTF-8 in our corpus; cp1252 is the obvious other candidate but
//! produces unreadable mojibake for the cp1251 majority. We pick
//! one fallback and live with it. When that's wrong, the metadata
//! string will look like garbage but the track itself will parse
//! cleanly — a future encoding-detection pass can transcode the
//! metadata field without touching geometry.

use encoding_rs::WINDOWS_1251;

/// Decode IGC source bytes to a UTF-8 `String`. Never fails: malformed
/// UTF-8 is reinterpreted as Windows-1251.
pub fn decode_text(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    let (cow, _encoding, _had_errors) = WINDOWS_1251.decode(bytes);
    cow.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_ascii_passes_through_unchanged() {
        let input =
            b"AXGD Test recorder\nHFDTEDATE:030526,01\nB1052024646349N01308989EA0166401735\n";
        let decoded = decode_text(input);
        assert_eq!(decoded.as_bytes(), input);
    }

    #[test]
    fn valid_utf8_is_preserved() {
        // 'pilot José' — valid UTF-8, must round-trip exactly.
        let input = "HFPLT:Jos\u{00e9}\n".as_bytes();
        assert_eq!(decode_text(input), "HFPLT:José\n");
    }

    /// Pilot field encoded in Windows-1251 (Cyrillic). The decode
    /// must produce the correct UTF-8 string, not mojibake.
    #[test]
    fn cp1251_pilot_field_decodes_to_correct_cyrillic() {
        // "HOPLTPILOT: " then the cp1251 bytes for "Илья" (a common
        // Russian first name): 0xC8 0xEB 0xFC 0xFF, then newline.
        let mut bytes: Vec<u8> = b"HOPLTPILOT: ".to_vec();
        bytes.extend_from_slice(&[0xC8, 0xEB, 0xFC, 0xFF]);
        bytes.push(b'\n');
        let decoded = decode_text(&bytes);
        assert_eq!(decoded, "HOPLTPILOT: \u{0418}\u{043B}\u{044C}\u{044F}\n");
    }

    #[test]
    fn ascii_lines_alongside_cp1251_metadata_still_parse() {
        // Mixed file: an A-record (ASCII), an H-record with cp1251
        // pilot name, then a B-record (ASCII). After decoding the
        // B-record bytes must be byte-identical to the input.
        let mut bytes: Vec<u8> = b"AXGD Test\nHOPLTPILOT: ".to_vec();
        bytes.extend_from_slice(&[0xC8, 0xEB, 0xFC, 0xFF]);
        bytes.extend_from_slice(b"\nB1052024646349N01308989EA0166401735\n");
        let decoded = decode_text(&bytes);
        assert!(decoded.contains("HOPLTPILOT: \u{0418}\u{043B}\u{044C}\u{044F}"));
        assert!(decoded.contains("B1052024646349N01308989EA0166401735"));
    }
}
