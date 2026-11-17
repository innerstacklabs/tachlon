//! The typed field readers [`read`](super::read) is built from. Each returns `Err(message)`
//! where the message starts with the key (`remaining: expected an integer, found "x"`), and the
//! caller puts the path in front of it with [`within`].

use super::json_read::Value;

pub(crate) type Field<T> = Result<T, String>;

/// `path` and the message of a field below it: `path.key: ...`, or `path: expected ...`.
pub(super) fn within(path: &str, error: &str) -> String {
    if error.starts_with("expected") {
        format!("{path}: {error}")
    } else {
        format!("{path}.{error}")
    }
}

/// The members of object `value` under `keys`, in the order of `keys`, in any order in the text.
/// An unknown key, a missing key and a key given twice are refused.
pub(crate) fn fields<'a, const N: usize>(
    value: &'a Value<'a>,
    keys: &[&str; N],
) -> Field<[&'a Value<'a>; N]> {
    fields_allowing(value, keys, &[])
}

/// [`fields`], and a key in `allowed` may also be present: it is not returned, so read it with
/// [`Value::get`].
pub(crate) fn fields_allowing<'a, const N: usize>(
    value: &'a Value<'a>,
    keys: &[&str; N],
    allowed: &[&str],
) -> Field<[&'a Value<'a>; N]> {
    let Value::Object(members) = value else {
        return Err(format!("expected an object, found {}", value.describe()));
    };
    let known = |key: &str| keys.contains(&key) || allowed.contains(&key);
    if let Some((key, _)) = members.iter().find(|(k, _)| !known(k)) {
        return Err(format!("{key}: unknown key"));
    }
    let mut found = [value; N];
    for (slot, key) in found.iter_mut().zip(keys) {
        let mut matching = members.iter().filter(|(k, _)| k == key);
        *slot = &matching
            .next()
            .ok_or_else(|| format!("{key}: missing key"))?
            .1;
        if matching.next().is_some() {
            return Err(format!("{key}: given twice"));
        }
    }
    Ok(found)
}

pub(crate) fn int(value: &Value, key: &str) -> Field<i64> {
    value
        .as_int()
        .ok_or_else(|| format!("{key}: expected an integer, found {}", value.describe()))
}

pub(crate) fn small<T: TryFrom<i64>>(value: &Value, key: &str) -> Field<T> {
    let n = int(value, key)?;
    T::try_from(n).map_err(|_| format!("{key}: {n} is out of range"))
}

pub(crate) fn str_of<'a>(value: &'a Value, key: &str) -> Field<&'a str> {
    value
        .as_str()
        .ok_or_else(|| format!("{key}: expected a string, found {}", value.describe()))
}

pub(crate) fn text_of(value: &Value, key: &str) -> Field<String> {
    str_of(value, key).map(str::to_string)
}

pub(crate) fn list<'a>(value: &'a Value<'a>, key: &str) -> Field<&'a [Value<'a>]> {
    value
        .as_array()
        .ok_or_else(|| format!("{key}: expected an array, found {}", value.describe()))
}

/// `None` for `null`, else what `read` makes of the value.
pub(crate) fn opt<T>(value: &Value, read: impl FnOnce(&Value) -> Field<T>) -> Field<Option<T>> {
    match value {
        Value::Null => Ok(None),
        other => read(other).map(Some),
    }
}

/// The bytes padded standard base64 `text` codes, or the offset of the first bad character.
pub(super) fn decode(text: &str) -> Result<Vec<u8>, usize> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(bytes.len());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (q, quad) in bytes.chunks(4).enumerate() {
        let last = (q + 1) * 4 == bytes.len();
        let pad = if last {
            quad.iter().rev().take_while(|&&b| b == b'=').count()
        } else {
            0
        };
        if pad > 2 {
            return Err(q * 4);
        }
        let mut n = 0u32;
        for (i, &b) in quad[..4 - pad].iter().enumerate() {
            let sextet = match b {
                b'A'..=b'Z' => b - b'A',
                b'a'..=b'z' => b - b'a' + 26,
                b'0'..=b'9' => b - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return Err(q * 4 + i),
            };
            n |= u32::from(sextet) << (18 - 6 * i);
        }
        out.extend_from_slice(&n.to_be_bytes()[1..4 - pad]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decode_reads_padded_base64_and_names_a_bad_offset() {
        assert_eq!(decode(""), Ok(vec![]));
        assert_eq!(decode("Zg=="), Ok(b"f".to_vec()));
        assert_eq!(decode("Zm8="), Ok(b"fo".to_vec()));
        assert_eq!(decode("Zm9vYmFy"), Ok(b"foobar".to_vec()));
        assert_eq!(decode("+//+"), Ok(vec![0xfb, 0xff, 0xfe]));
        assert_eq!(decode("Zm9"), Err(3));
        assert_eq!(decode("Zm!v"), Err(2));
        assert_eq!(decode("Zg==Zg=="), Err(2));
    }
}
