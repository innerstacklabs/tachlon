//! The legend strip in the footer of a printed page (gs#703): a swatch and a label for each thing
//! the chart draws, each swatch the light-theme colour of its token.

use super::page::esc;
use crate::texts::{self as t, Text, Texts};
use std::fmt::Write;
use timeline::Theme;

/// The legend entries: the label's text and the theme token whose colour the swatch has.
const ENTRIES: [(&Text, &str); 11] = [
    (&t::LEGEND_TASK, "bar"),
    (&t::LEGEND_CRITICAL, "crit"),
    (&t::LEGEND_DEADLINE, "crit"),
    (&t::LEGEND_START_CONSTRAINT, "text"),
    (&t::LEGEND_FINISH_CONSTRAINT, "text"),
    (&t::LEGEND_SUMMARY, "sum"),
    (&t::LEGEND_DEPENDENCY, "link"),
    (&t::LEGEND_DATA_DATE, "today"),
    (&t::LEGEND_NON_WORKING_DAY, "nonwork"),
    (&t::LEGEND_PROPOSAL_GHOST, "sel-edge"),
    (&t::LEGEND_JUST_MOVED, "sel-edge"),
];

const SWATCH: f64 = 10.0;
const GAP: f64 = 6.0;
const CHAR_W: f64 = 5.6;
const ENTRY_GAP: f64 = 10.0;
const FONT: f64 = 10.0;

/// `#rrggbb` of `color`; the alpha is written as `fill-opacity` by the caller.
fn hex(color: [f32; 4]) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "each component is clamped to 0 to 1 and scaled by 255, so the rounded byte is 0 to 255"
    )]
    let [r, g, b, _] = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Append the legend to `out` in the words of `texts`, its swatches' top edge at `y` and its first swatch at `x`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a label's byte length, far under 2^53, so f64 holds it exactly"
)]
pub(super) fn add_legend(out: &mut String, texts: &Texts, x: f64, y: f64) {
    let theme = Theme::light();
    let rule = hex(theme.rule);
    let ink = hex(theme.text);
    let mut at = x;
    for (text, token) in ENTRIES {
        let label = texts.say(text, &[]);
        let color = theme.get(token).expect("legend tokens are theme tokens");
        let _ = write!(
            out,
            "<rect class=\"legend-swatch\" x=\"{at}\" y=\"{y}\" width=\"{SWATCH}\" height=\"{SWATCH}\" fill=\"{}\"",
            hex(color)
        );
        if color[3] < 1.0 {
            let _ = write!(out, " fill-opacity=\"{}\"", color[3]);
        }
        let _ = write!(out, " stroke=\"{rule}\" stroke-width=\"0.5\"/>");
        at += SWATCH + GAP;
        let _ = write!(
            out,
            "<text x=\"{at}\" y=\"{}\" font-size=\"{FONT}\" fill=\"{ink}\">{}</text>",
            y + SWATCH - 1.0,
            esc(&label)
        );
        at += label.len() as f64 * CHAR_W + ENTRY_GAP;
    }
}
