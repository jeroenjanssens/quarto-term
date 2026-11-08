//! Decoding PTY output that arrives in chunks.
//!
//! The PTY reader delivers output in chunks of arbitrary size, so a multibyte
//! UTF-8 character can be split between two chunks. Decoding each chunk on its
//! own (`String::from_utf8_lossy`) turns both halves into U+FFFD, which makes
//! the line one column wider and can make the terminal wrap it.

/// Decode a chunk of PTY output.
///
/// An incomplete UTF-8 sequence at the end of the chunk (at most 3 bytes) is
/// kept in `pending` and decoded together with the next chunk. Invalid bytes
/// become U+FFFD, like `String::from_utf8_lossy`.
pub fn decode_utf8_chunk(pending: &mut Vec<u8>, bytes: &[u8]) -> String {
    pending.extend_from_slice(bytes);
    let mut out = String::with_capacity(pending.len());
    let mut rest: &[u8] = pending;
    loop {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                out.push_str(s);
                rest = &[];
                break;
            }
            Err(e) => {
                let (valid, after) = rest.split_at(e.valid_up_to());
                // Valid by construction: from_utf8 checked these bytes
                out.push_str(std::str::from_utf8(valid).unwrap_or_default());
                match e.error_len() {
                    // Invalid bytes: replace them and continue after them
                    Some(len) => {
                        out.push('\u{FFFD}');
                        rest = &after[len..];
                    }
                    // Incomplete sequence at the end: keep it for the next chunk
                    None => {
                        rest = after;
                        break;
                    }
                }
            }
        }
    }
    *pending = rest.to_vec();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode `chunks` one after the other, as the PTY reader would deliver them.
    fn decode_all(chunks: &[&[u8]]) -> (String, Vec<u8>) {
        let mut pending = Vec::new();
        let out = chunks.iter().map(|c| decode_utf8_chunk(&mut pending, c)).collect();
        (out, pending)
    }

    #[test]
    fn ascii_passes_through() {
        assert_eq!(decode_all(&[b"hello", b" world"]), ("hello world".to_string(), vec![]));
    }

    #[test]
    fn empty_chunks() {
        assert_eq!(decode_all(&[b"", b"a", b""]), ("a".to_string(), vec![]));
    }

    #[test]
    fn three_byte_char_split_after_first_byte() {
        let bytes = "a─b".as_bytes(); // ─ is E2 94 80
        let mut pending = Vec::new();
        assert_eq!(decode_utf8_chunk(&mut pending, &bytes[..2]), "a");
        assert_eq!(pending, vec![0xE2]);
        assert_eq!(decode_utf8_chunk(&mut pending, &bytes[2..]), "─b");
        assert!(pending.is_empty());
    }

    #[test]
    fn three_byte_char_split_after_second_byte() {
        let bytes = "─".as_bytes();
        assert_eq!(decode_all(&[&bytes[..2], &bytes[2..]]), ("─".to_string(), vec![]));
    }

    #[test]
    fn four_byte_char_split_into_single_bytes() {
        let b = "😀".as_bytes();
        assert_eq!(decode_all(&[&b[..1], &b[1..2], &b[2..3], &b[3..]]), ("😀".to_string(), vec![]));
    }

    #[test]
    fn split_at_every_position() {
        let text = "┌─┬─┐ ✓ é 😀 ascii";
        let bytes = text.as_bytes();
        for i in 0..=bytes.len() {
            let (out, pending) = decode_all(&[&bytes[..i], &bytes[i..]]);
            assert_eq!(out, text, "split at {i}");
            assert!(pending.is_empty(), "nothing pending after split at {i}");
        }
    }

    #[test]
    fn one_byte_at_a_time() {
        let text = "──── ✓é😀 done";
        let mut pending = Vec::new();
        let out: String = text
            .as_bytes()
            .iter()
            .map(|b| decode_utf8_chunk(&mut pending, &[*b]))
            .collect();
        assert_eq!(out, text);
        assert!(pending.is_empty());
    }

    #[test]
    fn chunks_of_4096_bytes_of_box_drawing() {
        // The PTY reader's buffer size; 4096 isn't a multiple of 3
        let text = "─".repeat(10_000);
        let chunks: Vec<&[u8]> = text.as_bytes().chunks(4096).collect();
        let (out, pending) = decode_all(&chunks);
        assert_eq!(out, text);
        assert!(pending.is_empty());
    }

    #[test]
    fn invalid_byte_is_replaced() {
        assert_eq!(decode_all(&[b"a\xffb"]), ("a\u{FFFD}b".to_string(), vec![]));
    }

    #[test]
    fn invalid_bytes_match_from_utf8_lossy() {
        let inputs: [&[u8]; 5] = [b"\xff\xfe", b"a\x80b", b"\xe2\x94x", b"\xf0\x9f\x98", b"\xed\xa0\x80"];
        for input in inputs {
            // In one chunk, followed by ASCII, so nothing is left pending
            let mut bytes = input.to_vec();
            bytes.extend_from_slice(b"!");
            assert_eq!(
                decode_all(&[&bytes]).0,
                String::from_utf8_lossy(&bytes),
                "input {input:?}"
            );
        }
    }

    #[test]
    fn truncated_sequence_followed_by_ascii_is_replaced() {
        // E2 94 starts a 3-byte character, but "x" isn't a continuation byte
        assert_eq!(decode_all(&[b"\xe2\x94", b"x"]), ("\u{FFFD}x".to_string(), vec![]));
    }

    #[test]
    fn incomplete_sequence_stays_pending() {
        let (out, pending) = decode_all(&[b"ok\xf0\x9f"]);
        assert_eq!(out, "ok");
        assert_eq!(pending, vec![0xF0, 0x9F]);
    }

    #[test]
    fn escape_sequences_are_kept() {
        let text = "\x1b[32m─\x1b[0m";
        let bytes = text.as_bytes();
        for i in 0..=bytes.len() {
            assert_eq!(decode_all(&[&bytes[..i], &bytes[i..]]).0, text, "split at {i}");
        }
    }
}
