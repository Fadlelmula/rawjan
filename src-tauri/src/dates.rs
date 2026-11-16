use chrono::{Datelike, Duration, Local, NaiveDate, NaiveTime, TimeZone, Utc};

use crate::error::{AppError, AppResult};

pub fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn today_local() -> NaiveDate {
    Local::now().date_naive()
}

pub fn parse_date(value: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("Invalid date '{value}'. Expected YYYY-MM-DD.")))
}

pub fn iso(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

pub fn month_key(date: NaiveDate) -> String {
    date.format("%Y-%m").to_string()
}

/// Convert a local calendar date (and optional local time) into a UTC instant string.
pub fn local_instant(date: NaiveDate, time: Option<&str>) -> AppResult<String> {
    let parsed_time = match time {
        Some(raw) if !raw.trim().is_empty() => NaiveTime::parse_from_str(raw.trim(), "%H:%M")
            .map_err(|_| AppError::Validation(format!("Invalid time '{raw}'. Expected HH:MM.")))?,
        _ => NaiveTime::from_hms_opt(12, 0, 0).expect("valid noon"),
    };
    let naive = date.and_time(parsed_time);
    match Local.from_local_datetime(&naive).earliest() {
        Some(dt) => Ok(dt
            .with_timezone(&Utc)
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()),
        None => Err(AppError::Validation(
            "The selected local date and time does not exist in this timezone.".into(),
        )),
    }
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1).expect("valid month");
    first_next.pred_opt().expect("valid date").day()
}

/// Return the given day clamped to the last day of the month.
pub fn clamp_day(year: i32, month: u32, day: u32) -> NaiveDate {
    let day = day.clamp(1, days_in_month(year, month));
    NaiveDate::from_ymd_opt(year, month, day).expect("valid clamped date")
}

pub fn add_months_clamped(date: NaiveDate, months: i32) -> NaiveDate {
    let total = date.year() * 12 + date.month0() as i32 + months;
    let year = total.div_euclid(12);
    let month = (total.rem_euclid(12) + 1) as u32;
    clamp_day(year, month, date.day())
}

/// Budget cycle that starts on `start_day` and ends the day before the next cycle start.
/// Returns an inclusive (start, end) range.
pub fn cycle_period(date: NaiveDate, start_day: u32) -> (NaiveDate, NaiveDate) {
    let start_day = start_day.clamp(1, 31);
    let this_start = clamp_day(date.year(), date.month(), start_day);
    if date >= this_start {
        let next_month = month_start(this_start, 1);
        let next_start = clamp_day(next_month.year(), next_month.month(), start_day);
        (this_start, next_start - Duration::days(1))
    } else {
        let prev_month = month_start(this_start, -1);
        let prev_start = clamp_day(prev_month.year(), prev_month.month(), start_day);
        (prev_start, this_start - Duration::days(1))
    }
}

pub fn calendar_month(date: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).expect("valid date");
    let end = NaiveDate::from_ymd_opt(
        date.year(),
        date.month(),
        days_in_month(date.year(), date.month()),
    )
    .expect("valid date");
    (start, end)
}

pub fn month_start(date: NaiveDate, offset_months: i32) -> NaiveDate {
    let total = date.year() * 12 + date.month0() as i32 + offset_months;
    let year = total.div_euclid(12);
    let month = (total.rem_euclid(12) + 1) as u32;
    NaiveDate::from_ymd_opt(year, month, 1).expect("valid month start")
}

/// Resolve a named reporting period into an inclusive date range.
pub fn period_range(
    key: &str,
    reference: NaiveDate,
    cycle_start_day: u32,
) -> AppResult<(NaiveDate, NaiveDate)> {
    let today = reference;
    Ok(match key {
        "this_month" => calendar_month(today),
        "last_month" => calendar_month(month_start(today, -1)),
        "last_3_months" => {
            let start = month_start(today, -2);
            (start, calendar_month(today).1)
        }
        "last_6_months" => {
            let start = month_start(today, -5);
            (start, calendar_month(today).1)
        }
        "last_12_months" => {
            let start = month_start(today, -11);
            (start, calendar_month(today).1)
        }
        "this_year" => (
            NaiveDate::from_ymd_opt(today.year(), 1, 1).expect("valid year"),
            NaiveDate::from_ymd_opt(today.year(), 12, 31).expect("valid year"),
        ),
        "current_cycle" => cycle_period(today, cycle_start_day),
        "last_cycle" => cycle_period(add_months_clamped(today, -1), cycle_start_day),
        "all_time" => (
            NaiveDate::from_ymd_opt(1900, 1, 1).expect("valid date"),
            today,
        ),
        other => {
            return Err(AppError::Validation(format!(
                "Unknown period '{other}'. Use this_month, last_month, last_3_months, last_6_months, last_12_months, this_year, current_cycle, last_cycle or all_time."
            )))
        }
    })
}

pub fn each_month(start: NaiveDate, end: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut result = Vec::new();
    let mut cursor = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).expect("valid date");
    while cursor <= end {
        let month_end = calendar_month(cursor).1;
        result.push((cursor.max(start), month_end.min(end)));
        cursor = add_months_clamped(
            NaiveDate::from_ymd_opt(cursor.year(), cursor.month(), 1).expect("valid"),
            1,
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn cycle_starting_on_28_runs_through_27() {
        assert_eq!(
            cycle_period(d(2026, 11, 14), 28),
            (d(2026, 10, 28), d(2026, 11, 27))
        );
        assert_eq!(
            cycle_period(d(2026, 11, 28), 28),
            (d(2026, 11, 28), d(2026, 12, 27))
        );
        assert_eq!(
            cycle_period(d(2026, 11, 27), 28),
            (d(2026, 10, 28), d(2026, 11, 27))
        );
    }

    #[test]
    fn cycle_handles_february_and_31st() {
        assert_eq!(
            cycle_period(d(2026, 3, 1), 31),
            (d(2026, 2, 28), d(2026, 3, 30))
        );
        assert_eq!(
            cycle_period(d(2026, 3, 31), 31),
            (d(2026, 3, 31), d(2026, 4, 29))
        );
        assert_eq!(
            cycle_period(d(2028, 2, 29), 30),
            (d(2028, 2, 29), d(2028, 3, 29))
        );
    }

    #[test]
    fn calendar_month_is_inclusive() {
        assert_eq!(
            calendar_month(d(2026, 2, 10)),
            (d(2026, 2, 1), d(2026, 2, 28))
        );
        assert_eq!(
            calendar_month(d(2028, 2, 10)),
            (d(2028, 2, 1), d(2028, 2, 29))
        );
    }

    #[test]
    fn add_months_clamps_to_month_end() {
        assert_eq!(add_months_clamped(d(2026, 1, 31), 1), d(2026, 2, 28));
        assert_eq!(add_months_clamped(d(2026, 3, 31), -1), d(2026, 2, 28));
        assert_eq!(add_months_clamped(d(2024, 2, 29), 12), d(2025, 2, 28));
    }

    #[test]
    fn month_enumeration_covers_partial_ranges() {
        let months = each_month(d(2026, 1, 15), d(2026, 3, 10));
        assert_eq!(months.len(), 3);
        assert_eq!(months[0], (d(2026, 1, 15), d(2026, 1, 31)));
        assert_eq!(months[2], (d(2026, 3, 1), d(2026, 3, 10)));
    }

    #[test]
    fn unknown_period_is_rejected() {
        assert!(period_range("whenever", d(2026, 1, 1), 28).is_err());
    }
}
