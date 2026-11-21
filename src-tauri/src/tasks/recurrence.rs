use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};

use super::BoardResult;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Days,
    Weeks,
    Months,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub frequency: Frequency,
    pub interval: u16,
    pub weekdays: Vec<u8>,
    pub month_day: Option<u8>,
    pub end_date: Option<String>,
}

pub(super) fn date(value: &str) -> Option<NaiveDate> {
    if value.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .filter(|day| (1..=9999).contains(&day.year()))
}

impl Rule {
    pub(super) fn validate(&self, anchor: NaiveDate) -> BoardResult<()> {
        if self.interval == 0 {
            return Err("Repeat interval must be at least one.".into());
        }
        if self.weekdays.iter().any(|day| *day > 6) {
            return Err("Select valid weekdays.".into());
        }
        if self.month_day.is_some_and(|day| !(1..=31).contains(&day)) {
            return Err("Enter a day from 1 to 31.".into());
        }
        if let Some(end) = &self.end_date {
            if !date(end).is_some_and(|end| end >= anchor) {
                return Err("Choose an end date on or after the recurrence start.".into());
            }
        }
        Ok(())
    }

    pub(super) fn next_on_or_after(
        &self,
        anchor: NaiveDate,
        first: NaiveDate,
    ) -> Option<NaiveDate> {
        let first = first.max(anchor);
        let interval = i64::from(self.interval);
        if interval == 0 {
            return None;
        }
        let candidate = match self.frequency {
            Frequency::Days => {
                let elapsed = first.signed_duration_since(anchor).num_days();
                anchor.checked_add_days(Days::new(round_up(elapsed, interval) as u64))
            }
            Frequency::Weeks => self.next_weekday(anchor, first, interval),
            Frequency::Months => self.next_month_day(anchor, first, interval),
        }?;
        let end = self.end_date.as_deref().and_then(date);
        (candidate.year() <= 9999 && end.is_none_or(|end| candidate <= end)).then_some(candidate)
    }

    fn next_weekday(
        &self,
        anchor: NaiveDate,
        first: NaiveDate,
        interval: i64,
    ) -> Option<NaiveDate> {
        let weekday = anchor.weekday().num_days_from_monday() as u8;
        let monday = anchor.checked_sub_days(Days::new(u64::from(weekday)))?;
        let elapsed = first.signed_duration_since(monday).num_weeks();
        let week = round_up(elapsed, interval);
        for offset in [week, week + interval] {
            let start = monday.checked_add_days(Days::new((offset * 7) as u64))?;
            for day in 0..7 {
                let allowed = if self.weekdays.is_empty() {
                    day == weekday
                } else {
                    self.weekdays.contains(&day)
                };
                let candidate = start.checked_add_days(Days::new(u64::from(day)))?;
                if allowed && candidate >= first {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn next_month_day(
        &self,
        anchor: NaiveDate,
        first: NaiveDate,
        interval: i64,
    ) -> Option<NaiveDate> {
        let month = |date: NaiveDate| i64::from(date.year()) * 12 + i64::from(date.month0());
        let requested = self.month_day.map_or(anchor.day(), u32::from);
        let start = month(anchor) + round_up(month(first) - month(anchor), interval);
        // Gregorian month lengths repeat every 400 years. An interval can skip every month that has the requested day.
        for index in 0..4800 {
            let current = start + index * interval;
            let year = i32::try_from(current.div_euclid(12)).ok()?;
            if year > 9999 {
                return None;
            }
            if let Some(candidate) =
                NaiveDate::from_ymd_opt(year, current.rem_euclid(12) as u32 + 1, requested)
            {
                if candidate >= first {
                    return Some(candidate);
                }
            }
        }
        None
    }
}

fn round_up(value: i64, step: i64) -> i64 {
    ((value + step - 1) / step) * step
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(frequency: Frequency, interval: u16) -> Rule {
        Rule {
            frequency,
            interval,
            weekdays: vec![],
            month_day: None,
            end_date: None,
        }
    }

    #[test]
    fn intervals_stay_anchored_after_late_completion() {
        let rule = rule(Frequency::Days, 3);
        assert_eq!(
            rule.next_on_or_after(date("2026-11-02").unwrap(), date("2026-11-07").unwrap()),
            date("2026-11-08")
        );
    }

    #[test]
    fn weekly_rule_allows_selected_days_in_interval_weeks() {
        let mut rule = rule(Frequency::Weeks, 2);
        rule.weekdays = vec![0, 4];
        let anchor = date("2026-11-02").unwrap();
        assert_eq!(
            rule.next_on_or_after(anchor, date("2026-11-04").unwrap()),
            date("2026-11-06")
        );
        assert_eq!(
            rule.next_on_or_after(anchor, date("2026-11-07").unwrap()),
            date("2026-11-16")
        );
    }

    #[test]
    fn monthly_rule_skips_missing_days_and_includes_end_date() {
        let mut rule = rule(Frequency::Months, 1);
        rule.month_day = Some(31);
        rule.end_date = Some("2028-03-31".into());
        let anchor = date("2028-01-31").unwrap();
        assert_eq!(
            rule.next_on_or_after(anchor, date("2028-02-01").unwrap()),
            date("2028-03-31")
        );
        assert_eq!(
            rule.next_on_or_after(anchor, date("2028-04-01").unwrap()),
            None
        );
    }
}
