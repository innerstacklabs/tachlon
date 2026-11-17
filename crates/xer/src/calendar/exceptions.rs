use super::CalendarException;

impl CalendarException {
    /// The exception day as an ISO `YYYY-MM-DD` date.
    #[must_use]
    pub fn date(&self) -> String {
        // P6 day 0 is 1899-12-30; this is Hinnant's civil-from-days with the
        // epoch moved to 0000-03-01. Plain i64 arithmetic cannot overflow here.
        let shifted = i64::from(self.serial) + 693_899;
        let era = shifted.div_euclid(146_097);
        let day_of_era = shifted.rem_euclid(146_097);
        let year_of_era =
            (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_index = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_index + 2) / 5 + 1;
        let month = if month_index < 10 {
            month_index + 3
        } else {
            month_index - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        format!("{year:04}-{month:02}-{day:02}")
    }
}
