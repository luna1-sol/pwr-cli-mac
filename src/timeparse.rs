use anyhow::{Result, anyhow, bail};
use chrono::{Duration as ChronoDuration, NaiveDate, NaiveDateTime, NaiveTime};
use std::time::Duration;

pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        bail!("duration is empty");
    }
    if let Ok(min) = s.parse::<u64>() {
        return Ok(Duration::from_secs(min * 60));
    }
    let mut total = 0u64;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let n: u64 = num
            .parse()
            .map_err(|_| anyhow!("invalid duration: {s} (e.g. 30m, 1h30m, 45s)"))?;
        num.clear();
        total += n * match c {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            'd' => 86400,
            _ => bail!("unknown unit '{c}' in {s} (use s/m/h/d)"),
        };
    }
    if !num.is_empty() {
        bail!("missing unit at end of {s} (e.g. 30m, 1h30m)");
    }
    if total == 0 {
        bail!("duration must be greater than zero");
    }
    Ok(Duration::from_secs(total))
}

pub fn duration_arg(s: &str) -> Result<Duration, String> {
    parse_duration(s).map_err(|e| e.to_string())
}

pub fn parse_when(s: &str, now: NaiveDateTime) -> Result<NaiveDateTime> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('+') {
        let d = parse_duration(rest)?;
        return Ok(now + ChronoDuration::seconds(d.as_secs() as i64));
    }
    if let Some(rest) = s.strip_prefix("tomorrow") {
        let t = parse_time(rest.trim())?;
        return Ok((now.date() + ChronoDuration::days(1)).and_time(t));
    }
    if let Ok(t) = parse_time(s) {
        let today = now.date().and_time(t);
        return Ok(if today > now {
            today
        } else {
            today + ChronoDuration::days(1)
        });
    }
    let normalized = s.replace('T', " ").replace('/', "-");
    if let Some((date, time)) = normalized.split_once(' ') {
        let d = NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d")
            .map_err(|_| anyhow!("invalid date: {date} (e.g. 2026-12-24)"))?;
        return Ok(d.and_time(parse_time(time.trim())?));
    }
    bail!("invalid date/time: {s} (e.g. 07:30, +30m, tomorrow 07:30, 2026-12-24 07:30)")
}

pub fn parse_time(s: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .map_err(|_| anyhow!("invalid time: {s} (e.g. 07:30)"))
}

pub fn pmset_datetime(dt: NaiveDateTime) -> String {
    dt.format("%m/%d/%y %H:%M:%S").to_string()
}

pub fn pmset_weekdays(s: &str) -> Result<String> {
    let lower = s.to_lowercase();
    let mapped = match lower.as_str() {
        "daily" | "everyday" => return Ok("MTWRFSU".into()),
        "weekdays" => return Ok("MTWRF".into()),
        "weekends" => return Ok("SU".into()),
        _ => s.to_uppercase(),
    };
    if mapped.is_empty() || !mapped.chars().all(|c| "MTWRFSU".contains(c)) {
        bail!(
            "invalid weekdays: {s} (combination of MTWRFSU, or daily / weekdays / weekends; R=Thu, S=Sat, U=Sun)"
        );
    }
    Ok(mapped)
}

pub fn human_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    match (h, m, s) {
        (0, 0, s) => format!("{s}s"),
        (0, m, 0) => format!("{m}m"),
        (0, m, s) => format!("{m}m{s}s"),
        (h, 0, _) => format!("{h}h"),
        (h, m, _) => format!("{h}h{m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 8)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("90").unwrap().as_secs(), 90 * 60);
        assert_eq!(parse_duration("45s").unwrap().as_secs(), 45);
        assert_eq!(parse_duration("1h30m").unwrap().as_secs(), 5400);
        assert_eq!(parse_duration("1d").unwrap().as_secs(), 86400);
        assert!(parse_duration("1x").is_err());
        assert!(parse_duration("1h30").is_err());
        assert!(parse_duration("0m").is_err());
    }

    #[test]
    fn when() {
        let n = now();
        assert_eq!(
            pmset_datetime(parse_when("13:00", n).unwrap()),
            "11/08/26 13:00:00"
        );
        assert_eq!(
            pmset_datetime(parse_when("07:30", n).unwrap()),
            "11/09/26 07:30:00"
        );
        assert_eq!(
            pmset_datetime(parse_when("+30m", n).unwrap()),
            "11/08/26 12:30:00"
        );
        assert_eq!(
            pmset_datetime(parse_when("tomorrow 13:00", n).unwrap()),
            "11/09/26 13:00:00"
        );
        assert_eq!(
            pmset_datetime(parse_when("2026-12-24 07:30", n).unwrap()),
            "12/24/26 07:30:00"
        );
        assert!(parse_when("someday", n).is_err());
    }

    #[test]
    fn weekdays() {
        assert_eq!(pmset_weekdays("weekdays").unwrap(), "MTWRF");
        assert_eq!(pmset_weekdays("mwf").unwrap(), "MWF");
        assert!(pmset_weekdays("XYZ").is_err());
    }

    #[test]
    fn human() {
        assert_eq!(human_duration(Duration::from_secs(5400)), "1h30m");
        assert_eq!(human_duration(Duration::from_secs(1800)), "30m");
    }
}
