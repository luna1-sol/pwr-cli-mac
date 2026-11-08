use crate::exec::output;
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct Status {
    pub power_source: String,
    pub has_battery: bool,
    pub percent: Option<u32>,
    pub charging: Option<bool>,
    pub fully_charged: Option<bool>,
    pub external_connected: Option<bool>,
    pub time_remaining_min: Option<u32>,
    pub cycle_count: Option<u32>,
    pub design_cycle_count: Option<u32>,
    pub max_capacity_mah: Option<u32>,
    pub design_capacity_mah: Option<u32>,
    pub health_percent: Option<f64>,
    pub voltage_v: Option<f64>,
    pub amperage_ma: Option<i64>,
    pub watts: Option<f64>,
    pub temperature_c: Option<f64>,
    pub low_power_mode: Option<bool>,
    pub sleep_disabled: Option<bool>,
    pub sleep_prevented_by: Option<String>,
}

pub fn collect() -> Result<Status> {
    let batt = output("pmset", &["-g", "batt"])?;
    let ioreg = output("ioreg", &["-rn", "AppleSmartBattery"]).unwrap_or_default();
    let settings = output("pmset", &["-g"]).unwrap_or_default();
    Ok(parse(&batt, &ioreg, &settings))
}

fn parse(batt: &str, ioreg: &str, settings: &str) -> Status {
    let mut st = Status {
        power_source: batt.split('\'').nth(1).unwrap_or("unknown").to_string(),
        ..Default::default()
    };

    st.low_power_mode = setting_value(settings, "lowpowermode").map(|v| v.starts_with('1'));
    st.sleep_disabled = (!settings.trim().is_empty())
        .then(|| setting_value(settings, "SleepDisabled").is_some_and(|v| v.starts_with('1')));
    st.sleep_prevented_by = setting_value(settings, "sleep").and_then(|v| {
        let start = v.find("prevented by ")? + "prevented by ".len();
        Some(v[start..].trim_end_matches(')').to_string())
    });

    st.has_battery = batt.contains("InternalBattery") || find_num(ioreg, "CycleCount").is_some();
    if !st.has_battery {
        return st;
    }

    let cur = find_num(ioreg, "CurrentCapacity");
    let max = find_num(ioreg, "MaxCapacity");
    st.percent = match (cur, max) {
        (Some(c), Some(m)) if m > 0 => Some(((c as f64 * 100.0 / m as f64).round()) as u32),
        _ => percent_from_pmset(batt),
    };
    st.charging = find_bool(ioreg, "IsCharging");
    st.fully_charged = find_bool(ioreg, "FullyCharged");
    st.external_connected = find_bool(ioreg, "ExternalConnected");
    st.cycle_count = find_num(ioreg, "CycleCount").map(|v| v as u32);
    st.design_cycle_count = find_num(ioreg, "DesignCycleCount9C").map(|v| v as u32);

    st.max_capacity_mah = find_num(ioreg, "AppleRawMaxCapacity")
        .or(max.filter(|&m| m > 100))
        .map(|v| v as u32);
    st.design_capacity_mah = find_num(ioreg, "DesignCapacity").map(|v| v as u32);
    if let (Some(m), Some(d)) = (st.max_capacity_mah, st.design_capacity_mah)
        && d > 0
    {
        st.health_percent = Some(round1(m as f64 * 100.0 / d as f64));
    }

    st.voltage_v = find_num(ioreg, "Voltage").map(|mv| mv as f64 / 1000.0);
    st.amperage_ma = find_num(ioreg, "Amperage");
    if let (Some(v), Some(a)) = (st.voltage_v, st.amperage_ma) {
        st.watts = Some(round1(v * a as f64 / 1000.0));
    }
    st.temperature_c = find_num(ioreg, "Temperature").map(|t| round1(t as f64 / 100.0));

    let valid = |m: i64| (m > 0 && m < 65535).then_some(m as u32);
    st.time_remaining_min = match (st.external_connected, st.charging) {
        (Some(true), Some(true)) => find_num(ioreg, "AvgTimeToFull").and_then(valid),
        (Some(true), _) => None,
        _ => find_num(ioreg, "TimeRemaining")
            .and_then(valid)
            .or_else(|| find_num(ioreg, "AvgTimeToEmpty").and_then(valid)),
    };
    st
}

pub fn print(st: &Status) {
    let row = |k: &str, v: String| println!("{k:<18}{v}");
    let source = match st.power_source.as_str() {
        "AC Power" => "AC adapter".to_string(),
        "Battery Power" => "Battery".to_string(),
        other => other.to_string(),
    };
    row("Power source", source);

    if st.has_battery {
        if let Some(p) = st.percent {
            let state = match (st.charging, st.fully_charged, st.external_connected) {
                (_, Some(true), _) => "fully charged",
                (Some(true), _, _) => "charging",
                (_, _, Some(true)) => "not charging (optimized charging / charge limit)",
                _ => "discharging",
            };
            row("Charge", format!("{p:>3}% {}  {state}", bar(p)));
        }
        let time = match st.time_remaining_min {
            Some(m) => format!("{}h {:02}m", m / 60, m % 60),
            None if st.external_connected == Some(true) && st.charging != Some(true) => "-".into(),
            None => "calculating".into(),
        };
        let label = if st.charging == Some(true) {
            "Time to full"
        } else {
            "Time remaining"
        };
        row(label, time);
        if let Some(c) = st.cycle_count {
            let design = st
                .design_cycle_count
                .map(|d| format!(" / {d}"))
                .unwrap_or_default();
            row("Cycle count", format!("{c}{design}"));
        }
        if let (Some(m), Some(d), Some(h)) = (
            st.max_capacity_mah,
            st.design_capacity_mah,
            st.health_percent,
        ) {
            row("Max capacity", format!("{h}% ({m} / design {d} mAh)"));
        }
        if let Some(w) = st.watts.filter(|w| *w != 0.0) {
            let dir = if w > 0.0 { "charging" } else { "draining" };
            row("Power", format!("{:.1} W ({dir})", w.abs()));
        }
        if let Some(v) = st.voltage_v {
            row("Voltage", format!("{v:.2} V"));
        }
        if let Some(t) = st.temperature_c {
            row("Temperature", format!("{t:.1} °C"));
        }
    } else {
        row("Battery", "none".into());
    }

    if let Some(lp) = st.low_power_mode {
        row("Low Power Mode", if lp { "on" } else { "off" }.into());
    }
    if let Some(sd) = st.sleep_disabled {
        let v = if sd {
            "ON (never sleeps, even with the lid closed; `pwr nosleep off` to restore)"
        } else {
            "off"
        };
        row("Sleep disabled", v.into());
    }
    if let Some(by) = &st.sleep_prevented_by {
        row("Sleep blocked by", by.clone());
    }
}

fn bar(p: u32) -> String {
    let filled = (p.min(100) as usize + 5) / 10;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(10 - filled))
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn find_num(text: &str, key: &str) -> Option<i64> {
    for pat in [format!("\"{key}\" = "), format!("\"{key}\"=")] {
        if let Some(pos) = text.find(&pat) {
            let rest = &text[pos + pat.len()..];
            let end = rest
                .char_indices()
                .find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && c == '-')))
                .map_or(rest.len(), |(i, _)| i);
            if let Ok(v) = rest[..end].parse::<i128>() {
                return Some(if v > i64::MAX as i128 {
                    (v - (1i128 << 64)) as i64
                } else {
                    v as i64
                });
            }
        }
    }
    None
}

fn find_bool(text: &str, key: &str) -> Option<bool> {
    let pat = format!("\"{key}\" = ");
    let rest = &text[text.find(&pat)? + pat.len()..];
    if rest.starts_with("Yes") {
        Some(true)
    } else if rest.starts_with("No") {
        Some(false)
    } else {
        None
    }
}

fn percent_from_pmset(batt: &str) -> Option<u32> {
    let idx = batt.find('%')?;
    let digits: String = batt[..idx]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().ok()
}

fn setting_value<'a>(settings: &'a str, key: &str) -> Option<&'a str> {
    settings.lines().find_map(|l| {
        let l = l.trim_start();
        let rest = l.strip_prefix(key)?;
        rest.starts_with(char::is_whitespace).then(|| rest.trim())
    })
}

#[derive(Debug, Serialize)]
pub struct Assertion {
    pub pid: u32,
    pub process: String,
    pub kind: String,
    pub name: String,
    pub age: String,
}

const BLOCKING: &[&str] = &[
    "PreventUserIdleSystemSleep",
    "PreventSystemSleep",
    "PreventUserIdleDisplaySleep",
    "NoIdleSleepAssertion",
    "NoDisplaySleepAssertion",
];

pub fn assertions(all: bool) -> Result<Vec<Assertion>> {
    let text = output("pmset", &["-g", "assertions"])?;
    Ok(parse_assertions(&text)
        .into_iter()
        .filter(|a| all || BLOCKING.contains(&a.kind.as_str()))
        .collect())
}

fn parse_assertions(text: &str) -> Vec<Assertion> {
    let mut out = Vec::new();
    let mut in_list = false;
    for line in text.lines() {
        if line.starts_with("Listed by owning process") {
            in_list = true;
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_list = false;
        }
        let Some(rest) = line.trim().strip_prefix("pid ").filter(|_| in_list) else {
            continue;
        };
        let Some((head, tail)) = rest.split_once("): ") else {
            continue;
        };
        let Some((pid, process)) = head.split_once('(') else {
            continue;
        };
        let tail = tail.split_once("] ").map_or(tail, |(_, t)| t);
        let mut parts = tail.splitn(3, ' ');
        let age = parts.next().unwrap_or_default().to_string();
        let kind = parts.next().unwrap_or_default().to_string();
        let name = parts
            .next()
            .and_then(|s| s.strip_prefix("named: "))
            .map(|s| s.trim().trim_matches('"').to_string())
            .unwrap_or_default();
        out.push(Assertion {
            pid: pid.trim().parse().unwrap_or(0),
            process: process.to_string(),
            kind,
            name,
            age,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BATT: &str = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=23658595)\t80%; AC attached; not charging present: true\n";
    const IOREG: &str = r#"
  |   "TimeRemaining" = 65535
  |   "ExternalConnected" = Yes
  |   "Voltage" = 12474
  |   "MaxCapacity" = 100
  |   "CurrentCapacity" = 80
  |   "CycleCount" = 24
  |   "BatteryData" = {"CurrentCapacity"=80,"AppleRawMaxCapacity"=4667,"DesignCapacity"=4629}
  |   "AvgTimeToFull" = 65535
  |   "FullyCharged" = No
  |   "Amperage" = 18446744073709550616
  |   "IsCharging" = No
  |   "DesignCycleCount9C" = 1000
"#;
    const SETTINGS: &str =
        " sleep                1 (sleep prevented by Claude, powerd)\n lowpowermode         0\n";

    #[test]
    fn parse_status() {
        let st = parse(BATT, IOREG, SETTINGS);
        assert_eq!(st.power_source, "AC Power");
        assert!(st.has_battery);
        assert_eq!(st.percent, Some(80));
        assert_eq!(st.charging, Some(false));
        assert_eq!(st.external_connected, Some(true));
        assert_eq!(st.time_remaining_min, None);
        assert_eq!(st.cycle_count, Some(24));
        assert_eq!(st.max_capacity_mah, Some(4667));
        assert_eq!(st.design_capacity_mah, Some(4629));
        assert_eq!(st.health_percent, Some(100.8));
        assert_eq!(st.amperage_ma, Some(-1000));
        assert_eq!(st.low_power_mode, Some(false));
        assert_eq!(st.sleep_disabled, Some(false));
        assert_eq!(st.sleep_prevented_by.as_deref(), Some("Claude, powerd"));
    }

    #[test]
    fn sleep_disabled() {
        let on = parse(
            BATT,
            IOREG,
            " SleepDisabled\t\t1\n sleep                1\n",
        );
        assert_eq!(on.sleep_disabled, Some(true));
        assert_eq!(on.sleep_prevented_by, None);
        let unknown = parse(BATT, IOREG, "");
        assert_eq!(unknown.sleep_disabled, None);
    }

    #[test]
    fn desktop_without_battery() {
        let st = parse("Now drawing from 'AC Power'\n", "", "");
        assert!(!st.has_battery);
        assert_eq!(st.percent, None);
    }

    #[test]
    fn parse_assertion_lines() {
        let text = "Assertion status system-wide:\n   UserIsActive 1\nListed by owning process:\n   pid 19401(Claude): [0x0000f79a0001a3fe] 00:00:22 NoIdleSleepAssertion named: \"Electron\"  \n   pid 361(powerd): [0x0000f1fe0009a26e] 00:24:19 UserIsActive named: \"com.apple.powermanagement.lidopen\"  \n";
        let a = parse_assertions(text);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].pid, 19401);
        assert_eq!(a[0].process, "Claude");
        assert_eq!(a[0].kind, "NoIdleSleepAssertion");
        assert_eq!(a[0].name, "Electron");
        assert_eq!(a[0].age, "00:00:22");
    }
}
