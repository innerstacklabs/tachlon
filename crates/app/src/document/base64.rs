//! Standard-alphabet, padded base64 for the document's `source` field, with no dependency.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Append `bytes` to `out` as padded base64.
pub(super) fn encode_into(bytes: &[u8], out: &mut String) {
    out.reserve(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        #[expect(clippy::cast_lossless, reason = "u8 widens to u32 exactly")]
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        let sextet = |shift: u32| ALPHABET[(n >> shift & 63) as usize] as char;
        out.push(sextet(18));
        out.push(sextet(12));
        out.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        out.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
}

#[cfg(test)]
mod tests {
    use super::encode_into;

    fn encode(bytes: &[u8]) -> String {
        let mut out = String::new();
        encode_into(bytes, &mut out);
        out
    }

    #[test]
    fn rfc_4648_vectors() {
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode(plain.as_bytes()), coded);
        }
        assert_eq!(encode(&[0xfb, 0xff, 0xfe]), "+//+");
    }
}
