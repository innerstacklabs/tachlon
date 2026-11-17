//! Check 15: the chart's colours keep their meanings (tl#1193; the palette research by sagan,
//! 2026-11-02, in lifeos logs/research).
//!
//! In every colour block of `packages/gantt/src/styles/tokens.css` (the light default, the dark set by the
//! system, and the two forced sets):
//!
//! 1. No token holds a stock Tailwind palette value. Those values read as generated, and the
//!    product's palette is its own.
//! 2. Red means the critical path: a token whose colour is a saturated red is a `crit` token.
//! 3. Text is readable: `--gantt-text`, `--gantt-muted` and `--gantt-crit-text` reach 4.5:1
//!    (WCAG 2) on `--gantt-bg`, and `--gantt-sel-edge` and `--gantt-warn` reach it on `--gantt-sel`.
//!
//! It fails closed: a file with no colour block, a block without `--gantt-bg` or one of the text
//! tokens, a colour token that is not hex (`rgb()`, `hsl()`, a named colour), and a token declared
//! twice in a block, all fail. A translucent text colour is measured over its background, as it
//! shows; the backgrounds are read as opaque. A token set by `var()` is not a colour here and is
//! not read. Threat model: a good-faith author who picks a colour by eye.

use crate::architecture::support::read;

const TOKENS: &str = web!("src/styles/tokens.css");

/// Stock Tailwind values a generated page reaches for: the 300 to 700 steps of the hues the
/// palette is near (slate, gray, red, amber, emerald, teal, cyan, sky, blue, indigo, violet).
const TAILWIND: [&str; 55] = [
    "#cbd5e1", "#94a3b8", "#64748b", "#475569", "#334155", // slate
    "#d1d5db", "#9ca3af", "#6b7280", "#4b5563", "#374151", // gray
    "#fca5a5", "#f87171", "#ef4444", "#dc2626", "#b91c1c", // red
    "#fcd34d", "#fbbf24", "#f59e0b", "#d97706", "#b45309", // amber
    "#6ee7b7", "#34d399", "#10b981", "#059669", "#047857", // emerald
    "#5eead4", "#2dd4bf", "#14b8a6", "#0d9488", "#0f766e", // teal
    "#67e8f9", "#22d3ee", "#06b6d4", "#0891b2", "#0e7490", // cyan
    "#7dd3fc", "#38bdf8", "#0ea5e9", "#0284c7", "#0369a1", // sky
    "#93c5fd", "#60a5fa", "#3b82f6", "#2563eb", "#1d4ed8", // blue
    "#a5b4fc", "#818cf8", "#6366f1", "#4f46e5", "#4338ca", // indigo
    "#c4b5fd", "#a78bfa", "#8b5cf6", "#7c3aed", "#6d28d9", // violet
];

/// The text tokens that must reach 4.5:1 on the background.
const TEXT: [&str; 3] = ["--gantt-text", "--gantt-muted", "--gantt-crit-text"];

/// Text that also sits on the selection: a pressed toggle's label and a warning in a selected row.
const ON_SEL: [&str; 2] = ["--gantt-sel-edge", "--gantt-warn"];

/// CSS colour functions: a token that uses one is a colour the check cannot read, so it fails.
const COLOUR_FNS: [&str; 11] = [
    "rgb(",
    "rgba(",
    "hsl(",
    "hsla(",
    "hwb(",
    "lab(",
    "lch(",
    "oklab(",
    "oklch(",
    "color(",
    "color-mix(",
];

/// One colour block: its selector and its `--gantt-*` colour tokens, in order.
struct Block {
    selector: String,
    tokens: Vec<(String, String)>,
}

/// Whether a token's value is a colour: hex, a colour function, or a bare word (a named colour).
fn is_colour(value: &str) -> bool {
    value.starts_with('#')
        || COLOUR_FNS.iter().any(|f| value.starts_with(f))
        || (!value.is_empty() && value.chars().all(|c| c.is_ascii_alphabetic()))
}

/// The blocks that open on a `:where(.gantt` selector line and hold at least one colour token.
fn blocks(css: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut current: Option<Block> = None;
    for line in css.lines() {
        let line = line.trim();
        if line.contains(":where(.gantt") && line.ends_with('{') {
            current = Some(Block {
                selector: line.trim_end_matches('{').trim().to_string(),
                tokens: Vec::new(),
            });
        } else if line.starts_with('}') {
            out.extend(current.take().filter(|b| !b.tokens.is_empty()));
        } else if let Some(block) = current.as_mut()
            && let Some((name, value)) = line.trim_end_matches(';').split_once(':')
        {
            let value = value.trim().to_ascii_lowercase();
            if name.starts_with("--gantt-") && is_colour(&value) {
                block.tokens.push((name.to_string(), value));
            }
        }
    }
    out
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa` as 0..=1 channels, alpha last (1 when absent).
fn rgba(hex: &str) -> Option<[f64; 4]> {
    let h = hex.strip_prefix('#')?;
    let full: String = match h.len() {
        3 | 4 => h.chars().flat_map(|c| [c, c]).collect(),
        6 | 8 => h.to_string(),
        _ => return None,
    };
    let v = u32::from_str_radix(&format!("{full:f<8}"), 16).ok()?;
    Some([24, 16, 8, 0].map(|s| f64::from((v >> s) & 255) / 255.0))
}

/// The colour's channels with its alpha dropped.
fn rgb(hex: &str) -> Option<[f64; 3]> {
    rgba(hex).map(|[r, g, b, _]| [r, g, b])
}

/// `fg` composited over the opaque `bg`: a translucent text colour as it shows.
fn over([r, g, b, a]: [f64; 4], bg: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a * [r, g, b][i] + (1.0 - a) * bg[i])
}

/// WCAG 2 relative luminance.
fn luminance([r, g, b]: [f64; 3]) -> f64 {
    let lin = |c: f64| {
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// A saturated red: hue within 15 degrees of red, HSL saturation over 0.45, lightness 0.25 to 0.85.
#[expect(
    clippy::many_single_char_names,
    reason = "r, g, b, h, s, l, d are the colour terms"
)]
fn is_red([r, g, b]: [f64; 3]) -> bool {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = f64::midpoint(max, min);
    let d = max - min;
    if d == 0.0 || !(0.25..=0.85).contains(&l) {
        return false;
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let h = (h + 360.0) % 360.0;
    s > 0.45 && !(15.0..=345.0).contains(&h)
}

/// Every failure in `css`, one per broken rule and token.
fn check_palette(css: &str) -> Vec<String> {
    let blocks = blocks(css);
    if blocks.is_empty() {
        return vec![format!(
            "{TOKENS}: no colour block found; the check reads nothing"
        )];
    }
    let mut failures = Vec::new();
    for block in &blocks {
        let at = |name: &str| {
            block
                .tokens
                .iter()
                .rev()
                .find(|(n, _)| n == name)
                .and_then(|(_, v)| rgba(v))
        };
        for (i, (name, value)) in block.tokens.iter().enumerate() {
            let selector = &block.selector;
            if rgba(value).is_none() {
                failures.push(format!("{TOKENS} `{selector}`: {name} is {value}, not hex; the check reads hex colours only"));
            }
            if block.tokens[..i].iter().any(|(n, _)| n == name) {
                failures.push(format!("{TOKENS} `{selector}`: {name} is declared twice; CSS uses the last, so declare it once"));
            }
            if TAILWIND.contains(&value.as_str()) {
                failures.push(format!("{TOKENS} `{}`: {name} is {value}, a stock Tailwind value; pick the product's own", block.selector));
            }
            if rgb(value).is_some_and(is_red) && !name.contains("crit") {
                failures.push(format!(
                    "{TOKENS} `{}`: {name} is red ({value}); red means the critical path only",
                    block.selector
                ));
            }
        }
        let opaque = |name: &str| at(name).map(|[r, g, b, _]| [r, g, b]);
        let Some(bg) = opaque("--gantt-bg") else {
            failures.push(format!("{TOKENS} `{}`: no --gantt-bg", block.selector));
            continue;
        };
        if let Some(sel) = opaque("--gantt-sel") {
            for name in ON_SEL {
                let seen = at(name).map(|fg| contrast(over(fg, sel), sel));
                if let Some(ratio) = seen.filter(|r| *r < 4.5) {
                    failures.push(format!(
                        "{TOKENS} `{}`: {name} on --gantt-sel is {ratio:.2}:1, under 4.5:1",
                        block.selector
                    ));
                }
            }
        }
        for name in TEXT {
            match at(name).map(|fg| contrast(over(fg, bg), bg)) {
                None => failures.push(format!("{TOKENS} `{}`: no {name}", block.selector)),
                Some(ratio) if ratio < 4.5 => failures.push(format!(
                    "{TOKENS} `{}`: {name} on --gantt-bg is {ratio:.2}:1, under 4.5:1",
                    block.selector
                )),
                Some(_) => {}
            }
        }
    }
    failures
}

#[test]
fn the_chart_colours_keep_their_meanings() {
    let failures = check_palette(&read(TOKENS));
    assert!(
        failures.is_empty(),
        "the palette (check 15 in main.rs):\n  {}",
        failures.join("\n  ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = ":where(.gantt) {\n  --gantt-bg: #f2f5f9;\n  --gantt-text: #18213b;\n  --gantt-muted: #56628a;\n  --gantt-crit-text: #a82222;\n  --gantt-crit: #c42b2b;\n}\n";

    #[test]
    fn a_good_block_passes() {
        assert_eq!(check_palette(GOOD), Vec::<String>::new());
    }

    #[test]
    fn a_tailwind_value_fails() {
        let css = GOOD.replace(
            "--gantt-crit: #c42b2b;",
            "--gantt-crit: #c42b2b;\n  --gantt-today: #22d3ee;",
        );
        assert_eq!(check_palette(&css).len(), 1);
    }

    #[test]
    fn red_outside_the_critical_path_fails() {
        let css = GOOD.replace(
            "--gantt-crit: #c42b2b;",
            "--gantt-crit: #c42b2b;\n  --gantt-link-refused: #c8322b;",
        );
        assert!(check_palette(&css)[0].contains("red means the critical path"));
    }

    #[test]
    fn low_contrast_text_fails() {
        let css = GOOD.replace("#56628a", "#9aa6c6");
        assert!(check_palette(&css)[0].contains("--gantt-muted on --gantt-bg"));
    }

    #[test]
    fn an_empty_file_and_a_missing_token_fail() {
        assert_eq!(check_palette("").len(), 1);
        assert!(
            check_palette(&GOOD.replace("  --gantt-muted: #56628a;\n", ""))
                .iter()
                .any(|f| f.contains("no --gantt-muted"))
        );
    }

    #[test]
    fn text_on_the_selection_is_checked() {
        let css = GOOD.replace(
            "--gantt-crit: #c42b2b;",
            "--gantt-crit: #c42b2b;\n  --gantt-sel: #d9e2f6;\n  --gantt-sel-edge: #1f62c9;",
        );
        assert!(check_palette(&css)[0].contains("--gantt-sel-edge on --gantt-sel"));
    }

    #[test]
    fn a_colour_that_is_not_hex_fails() {
        let css = GOOD.replace("#c42b2b;", "#c42b2b;\n  --gantt-warn: rgb(200, 50, 43);");
        assert!(check_palette(&css)[0].contains("--gantt-warn is rgb(200, 50, 43), not hex"));
        let css =
            format!("{GOOD}:where(.gantt[data-theme=\"dark\"]) {{\n  --gantt-link: red;\n}}\n");
        assert!(
            check_palette(&css)
                .iter()
                .any(|f| f.contains("--gantt-link is red"))
        );
    }

    #[test]
    fn transparent_text_fails() {
        let css = GOOD.replace("#a82222;", "#a8222200;");
        assert!(check_palette(&css)[0].contains("--gantt-crit-text on --gantt-bg is 1.00:1"));
    }

    #[test]
    fn a_token_declared_twice_fails() {
        let css = GOOD.replace("#c42b2b;", "#c42b2b;\n  --gantt-text: #f2f5f9;");
        let failures = check_palette(&css);
        assert!(
            failures
                .iter()
                .any(|f| f.contains("--gantt-text is declared twice"))
        );
        assert!(
            failures
                .iter()
                .any(|f| f.contains("--gantt-text on --gantt-bg is 1.00:1"))
        );
    }

    #[test]
    fn contrast_matches_wcag() {
        let (black, white) = (rgb("#000").unwrap(), rgb("#ffffff").unwrap());
        assert!((contrast(black, white) - 21.0).abs() < 0.01);
        assert!(is_red(rgb("#c42b2b").unwrap()) && !is_red(rgb("#c47d00").unwrap()));
    }
}
