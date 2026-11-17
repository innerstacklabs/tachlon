//! A task's date rules as the two constraint slots of a `TASK` row (gantt#1875).

use engine::{Instant, Limit, Pin};

/// A `cstr_type` spelling and the instant of its date, which `CS_ALAP` has none of.
pub(super) type Pair = (&'static str, Option<Instant>);

/// The spellings a P6 probe file shows accepted in the second slot, `cstr_type2` with
/// `cstr_date2`: `CS_MSOA` and `CS_MEOA` (the `mss.xer` activities N1 and N7).
const SECOND: [&str; 2] = ["CS_MSOA", "CS_MEOA"];

/// The pairs a task's rules write, and whether a rule had no slot.
pub(super) struct Constraints {
    pub(super) pairs: Vec<Pair>,
    pub(super) dropped: bool,
}

/// The pairs for a task with `snet` (its start-no-earlier-than, when the file needs it) and
/// `limit`: the pin, then the spellings `SECOND` refuses (start-no-later-than, finish-no-later-than,
/// as late as possible), then the two it takes. The first fills the primary slot; the second fills
/// the secondary slot when a probe file shows that spelling accepted there. So a task keeps both
/// rules whenever at most one of them is a spelling `SECOND` refuses. A pin always comes first, so
/// it is always primary: P6 reads a mandatory date in the second slot as a soft one.
pub(super) fn constraints(snet: Option<Instant>, limit: Limit) -> Constraints {
    let pin = limit.pin.map(|pin| match pin {
        Pin::Start(at) => ("CS_MANDSTART", Some(at)),
        Pin::Finish(at) => ("CS_MANDFIN", Some(at)),
    });
    let rules = [
        pin,
        limit.snlt.map(|at| ("CS_MSOB", Some(at))),
        limit.fnlt.map(|at| ("CS_MEOB", Some(at))),
        limit.alap.then_some(("CS_ALAP", None)),
        snet.map(|at| ("CS_MSOA", Some(at))),
        limit.fnet.map(|at| ("CS_MEOA", Some(at))),
    ];
    let mut out = Constraints {
        pairs: Vec::new(),
        dropped: false,
    };
    for pair in rules.into_iter().flatten() {
        let fits = match out.pairs.len() {
            0 => true,
            1 => SECOND.contains(&pair.0),
            _ => false,
        };
        if fits {
            out.pairs.push(pair);
        } else {
            out.dropped = true;
        }
    }
    out
}
