//! Reading and writing the `#rgb`, `#rrggbb` and `#rrggbbaa` colour strings of a theme.

pub(super) fn format_color(color: [f32; 4]) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a colour component is 0 to 1, so the byte is 0 to 255"
    )]
    let byte = |component: f32| (component * 255.0).round() as u8;
    let [r, g, b, a] = color.map(byte);
    #[expect(
        clippy::float_cmp,
        reason = "an opaque colour holds alpha as exactly 1.0 (255 / 255.0 or the literal), and any other alpha, however close, is written with its alpha byte"
    )]
    let opaque = color[3] == 1.0;
    if opaque {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

pub(super) fn parse_color(value: &str) -> Option<[f32; 4]> {
    let hex = value.strip_prefix('#')?.as_bytes();
    let pair = |at| Some(hex_digit(*hex.get(at)?)? * 16 + hex_digit(*hex.get(at + 1)?)?);
    let (r, g, b, a) = match hex.len() {
        3 => {
            let digit = |at| hex_digit(hex[at]).map(|n| n * 17);
            (digit(0)?, digit(1)?, digit(2)?, u8::MAX)
        }
        6 => (pair(0)?, pair(2)?, pair(4)?, u8::MAX),
        8 => (pair(0)?, pair(2)?, pair(4)?, pair(6)?),
        _ => return None,
    };
    Some([
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        f32::from(a) / 255.0,
    ])
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
