//! A link's type and lag as a person types them (gs#703, story 9): `SS+2d`, `FF-1d`, `FS`.

use super::{DurationError, Edit, EditError, day_minutes, format_duration, parse_duration};
use crate::Session;
use crate::texts::{LINK_TEXT_KIND, LINK_TEXT_LAG, LINK_TEXT_SIGN, Texts, Worded, english_display};
use engine::{DepId, LagCalendar, LinkType, Minutes, TaskId};

/// Why a typed link was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkTextError {
    /// The text does not start with `FS`, `SS`, `FF` or `SF`.
    Kind,
    /// Something follows the type that is not a sign.
    Sign,
    /// The lag after the sign is not a duration.
    Lag(DurationError),
}

impl Worded for LinkTextError {
    fn text(&self, texts: &Texts) -> String {
        match self {
            Self::Kind => texts.say(&LINK_TEXT_KIND, &[]),
            Self::Sign => texts.say(&LINK_TEXT_SIGN, &[]),
            Self::Lag(error) => texts.say(&LINK_TEXT_LAG, &[("error", &error.text(texts))]),
        }
    }
}
english_display!(LinkTextError);

/// Parse `FS`, `SS+2d`, `FF-1d 4h`: the type letters (any case), then an optional lag with its
/// sign, in the duration text [`parse_duration`] reads, a day being `day_minutes`.
///
/// # Errors
/// A [`LinkTextError`] for a missing type, a bad sign, or a lag that is not a duration.
pub fn parse_link_text(
    text: &str,
    day_minutes: Minutes,
) -> Result<(LinkType, Minutes), LinkTextError> {
    let text = text.trim();
    let kind = match text.get(..2).map(str::to_ascii_uppercase).as_deref() {
        Some("FS") => LinkType::FS,
        Some("SS") => LinkType::SS,
        Some("FF") => LinkType::FF,
        Some("SF") => LinkType::SF,
        _ => return Err(LinkTextError::Kind),
    };
    let rest = text[2..].trim_start();
    let negative = match rest.chars().next() {
        None => return Ok((kind, 0)),
        Some('+') => false,
        Some('-') => true,
        Some(_) => return Err(LinkTextError::Sign),
    };
    let lag = parse_duration(&rest[1..], day_minutes).map_err(LinkTextError::Lag)?;
    Ok((kind, if negative { -lag } else { lag }))
}

/// A link's type and lag as [`parse_link_text`] reads them back: `FS`, `SS+2d`, `FF-1d 4h`. A zero
/// lag prints the type alone; a day is `day_minutes`.
#[must_use]
pub fn format_link_text(kind: LinkType, lag: Minutes, day_minutes: Minutes) -> String {
    let kind = match kind {
        LinkType::FS => "FS",
        LinkType::SS => "SS",
        LinkType::FF => "FF",
        LinkType::SF => "SF",
    };
    match lag {
        0 => kind.to_string(),
        lag => {
            let sign = if lag < 0 { '-' } else { '+' };
            let lag = format_duration(lag.abs(), day_minutes);
            format!("{kind}{sign}{lag}")
        }
    }
}

impl Session {
    /// Minutes in one "day" of the calendar `dep`'s lag is counted in: its own choice, else the
    /// schedule's. `dep` must be in range.
    pub(crate) fn lag_day_minutes(&self, dep: DepId) -> Minutes {
        let link = self.schedule.dependencies()[dep.0 as usize];
        let own = self.schedule.dependency_lag_calendar(dep);
        self.pair_day_minutes(link.pred, link.succ, own)
    }

    /// The same for a link from `pred` to `succ` with `own` as its calendar: a proposed one has no slot.
    pub(crate) fn pair_day_minutes(
        &self,
        pred: TaskId,
        succ: TaskId,
        own: Option<LagCalendar>,
    ) -> Minutes {
        let schedule = &self.schedule;
        let calendar = match own.unwrap_or(schedule.lag_calendar()) {
            LagCalendar::Successor => schedule.calendar_of(succ),
            LagCalendar::Predecessor => schedule.calendar_of(pred),
            LagCalendar::TwentyFourHour => return 1440,
            LagCalendar::Fixed(id) => &schedule.calendars()[id.0 as usize],
        };
        day_minutes(calendar)
    }

    /// Link `dep`'s type and lag as text (`SS+2d`), the lag in days of the calendar it is counted
    /// in, so [`Session::set_link_text`] sets the same link back. `None` for a removed or unknown
    /// link.
    pub fn link_text(&self, dep: DepId) -> Option<String> {
        if !self.schedule.is_dependency_live(dep) {
            return None;
        }
        let link = self.schedule.dependencies()[dep.0 as usize];
        Some(format_link_text(
            link.kind,
            link.lag,
            self.lag_day_minutes(dep),
        ))
    }

    /// Set a live link's type and lag from typed text, as one undo step ([`Edit::SetLink`]). A text
    /// that does not parse is refused with [`EditError::LinkText`]; nothing changes then.
    ///
    /// # Errors
    /// [`EditError::UnknownLink`] for a removed or unknown link, [`EditError::LinkText`] for text
    /// that does not parse, else the edit's refusal.
    pub fn set_link_text(&mut self, dep: DepId, text: &str) -> Result<(), EditError> {
        if !self.schedule.is_dependency_live(dep) {
            return Err(EditError::UnknownLink(dep));
        }
        let (kind, lag) =
            parse_link_text(text, self.lag_day_minutes(dep)).map_err(EditError::LinkText)?;
        self.edit(Edit::SetLink { dep, kind, lag })
    }
}
