mod exec;
mod status;
mod timeparse;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use exec::{Runner, confirm};
use std::time::Duration;
use timeparse::{duration_arg, human_duration};

#[derive(Parser)]
#[command(
    name = "pwr",
    version,
    about = "Power management CLI for macOS (wraps pmset / caffeinate / System Events)"
)]
struct Cli {
    #[arg(
        short = 'n',
        long,
        global = true,
        help = "Print the commands that would run without executing them"
    )]
    dry_run: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(visible_alias = "st", about = "Show battery and power status")]
    Status {
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    #[command(about = "Put the system to sleep now")]
    Sleep,
    #[command(visible_alias = "display-sleep", about = "Turn off the display only")]
    DisplayOff,
    #[command(about = "Shut down")]
    Shutdown(PowerOffArgs),
    #[command(about = "Reboot")]
    Reboot(PowerOffArgs),
    #[command(about = "Log out")]
    Logout {
        #[arg(short, long, help = "Skip confirmation")]
        yes: bool,
    },
    #[command(visible_alias = "caffeinate", about = "Prevent sleep (caffeinate)")]
    Awake(AwakeArgs),
    #[command(about = "Show processes preventing sleep")]
    Assertions {
        #[arg(short, long, help = "Include assertions that do not prevent sleep")]
        all: bool,
        #[arg(long, help = "Output as JSON")]
        json: bool,
    },
    #[command(
        subcommand,
        visible_alias = "sched",
        about = "Schedule wake / sleep / shutdown events (changes require sudo)"
    )]
    Schedule(ScheduleCmd),
    #[command(about = "Show power settings (pmset -g custom)")]
    Settings,
    #[command(about = "Change a power setting (sudo), e.g. pwr set displaysleep 10 --battery")]
    Set {
        #[arg(help = "pmset setting key (displaysleep, sleep, disksleep, powernap, womp, ...)")]
        key: String,
        #[arg(help = "Value to set")]
        value: String,
        #[command(flatten)]
        scope: Scope,
    },
    #[command(about = "Turn Low Power Mode on or off (sudo)")]
    LowPower {
        state: OnOff,
        #[command(flatten)]
        scope: Scope,
    },
}

#[derive(Args)]
struct PowerOffArgs {
    #[arg(short, long, help = "Skip confirmation")]
    yes: bool,
    #[arg(
        short,
        long,
        help = "Do not wait for apps to save; run immediately (uses sudo shutdown)"
    )]
    force: bool,
    #[arg(
        long = "in",
        value_name = "DURATION",
        value_parser = duration_arg,
        help = "Run after a delay (e.g. 30m, 1h); uses sudo shutdown"
    )]
    after: Option<Duration>,
}

#[derive(Args)]
struct AwakeArgs {
    #[arg(
        value_parser = duration_arg,
        conflicts_with = "command",
        help = "How long to stay awake (e.g. 30m, 2h, 1h30m; bare numbers are minutes). Until Ctrl-C if omitted"
    )]
    duration: Option<Duration>,
    #[arg(short, long, help = "Also keep the display awake")]
    display: bool,
    #[arg(
        short,
        long,
        help = "Prevent system sleep entirely while on AC power (not when the lid is closed)"
    )]
    system: bool,
    #[arg(
        last = true,
        value_name = "COMMAND",
        help = "Stay awake only while this command runs (e.g. pwr awake -- cargo build)"
    )]
    command: Vec<String>,
}

#[derive(Subcommand)]
enum ScheduleCmd {
    #[command(visible_alias = "ls", about = "List scheduled events")]
    List,
    #[command(about = "Add a one-time event, e.g. pwr schedule add wake 07:30 / add shutdown +2h")]
    Add {
        kind: EventKind,
        #[arg(
            required = true,
            num_args = 1..,
            help = "Date/time (07:30, +30m, tomorrow 07:30, 2026-12-24 07:30)"
        )]
        when: Vec<String>,
    },
    #[command(about = "Cancel a one-time event (use the same date/time as add)")]
    Cancel {
        kind: EventKind,
        #[arg(required = true, num_args = 1.., help = "Date/time")]
        when: Vec<String>,
    },
    #[command(about = "Cancel all one-time events")]
    Clear {
        #[arg(short, long, help = "Skip confirmation")]
        yes: bool,
    },
    #[command(
        about = "Set a weekly repeating event, e.g. pwr schedule repeat wakeorpoweron weekdays 08:00"
    )]
    Repeat {
        kind: EventKind,
        #[arg(help = "Days (combination of MTWRFSU / daily / weekdays / weekends)")]
        days: String,
        #[arg(help = "Time (HH:MM)")]
        time: String,
    },
    #[command(about = "Cancel all repeating events")]
    RepeatCancel,
}

#[derive(Clone, Copy, ValueEnum)]
enum EventKind {
    Wake,
    Sleep,
    Poweron,
    Shutdown,
    #[value(name = "reboot", alias = "restart")]
    Reboot,
    #[value(name = "wakeorpoweron")]
    WakeOrPowerOn,
}

impl EventKind {
    fn as_pmset(self) -> &'static str {
        match self {
            Self::Wake => "wake",
            Self::Sleep => "sleep",
            Self::Poweron => "poweron",
            Self::Shutdown => "shutdown",
            Self::Reboot => "restart",
            Self::WakeOrPowerOn => "wakeorpoweron",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum OnOff {
    On,
    Off,
}

#[derive(Args)]
struct Scope {
    #[arg(
        long,
        conflicts_with = "battery",
        help = "Apply only to the AC power profile"
    )]
    ac: bool,
    #[arg(long, help = "Apply only to the battery profile")]
    battery: bool,
}

impl Scope {
    fn flag(&self) -> &'static str {
        if self.ac {
            "-c"
        } else if self.battery {
            "-b"
        } else {
            "-a"
        }
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let r = Runner {
        dry_run: cli.dry_run,
    };
    match cli.cmd {
        Cmd::Status { json } => {
            let st = status::collect()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&st)?);
            } else {
                status::print(&st);
            }
        }
        Cmd::Sleep => r.run("pmset", &["sleepnow"])?,
        Cmd::DisplayOff => r.run("pmset", &["displaysleepnow"])?,
        Cmd::Shutdown(a) => power_off(&r, a, PowerOff::Shutdown)?,
        Cmd::Reboot(a) => power_off(&r, a, PowerOff::Reboot)?,
        Cmd::Logout { yes } => {
            if r.dry_run || confirm("Log out?", yes)? {
                r.osascript("tell application \"System Events\" to log out")?;
            }
        }
        Cmd::Awake(a) => awake(&r, a)?,
        Cmd::Assertions { all, json } => {
            let list = status::assertions(all)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&list)?);
            } else if list.is_empty() {
                println!("No processes are preventing sleep");
            } else {
                println!(
                    "{:<7} {:<22} {:<28} {:<9} NAME",
                    "PID", "PROCESS", "TYPE", "AGE"
                );
                for a in list {
                    println!(
                        "{:<7} {:<22} {:<28} {:<9} {}",
                        a.pid,
                        truncate(&a.process, 22),
                        a.kind,
                        a.age,
                        a.name
                    );
                }
            }
        }
        Cmd::Schedule(s) => schedule(&r, s)?,
        Cmd::Settings => print!("{}", exec::output("pmset", &["-g", "custom"])?),
        Cmd::Set { key, value, scope } => {
            r.run_root("pmset", &[scope.flag(), &key, &value])?;
        }
        Cmd::LowPower { state, scope } => {
            let v = match state {
                OnOff::On => "1",
                OnOff::Off => "0",
            };
            r.run_root("pmset", &[scope.flag(), "lowpowermode", v])?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PowerOff {
    Shutdown,
    Reboot,
}

fn power_off(r: &Runner, a: PowerOffArgs, kind: PowerOff) -> Result<()> {
    let (label, gerund, flag, script) = match kind {
        PowerOff::Shutdown => ("Shut down", "Shutting down", "-h", "shut down"),
        PowerOff::Reboot => ("Reboot", "Rebooting", "-r", "restart"),
    };
    let when = match a.after {
        Some(d) => format!("in {}", human_duration(d)),
        None => "now".into(),
    };
    if !r.dry_run && !confirm(&format!("{label} {when}?"), a.yes)? {
        println!("Cancelled");
        return Ok(());
    }
    match (a.after, a.force) {
        (Some(d), _) => {
            let minutes = d.as_secs().div_ceil(60).max(1);
            r.run_root("shutdown", &[flag, &format!("+{minutes}")])?;
            if !r.dry_run {
                println!("{gerund} in {minutes} minute(s). To cancel: sudo killall shutdown");
            }
        }
        (None, true) => r.run_root("shutdown", &[flag, "now"])?,
        (None, false) => r.osascript(&format!("tell application \"System Events\" to {script}"))?,
    }
    Ok(())
}

fn awake(r: &Runner, a: AwakeArgs) -> Result<()> {
    let mut args: Vec<String> = vec!["-i".into()];
    if a.display {
        args.push("-d".into());
    }
    if a.system {
        args.push("-s".into());
    }
    if let Some(d) = a.duration {
        args.push("-t".into());
        args.push(d.as_secs().to_string());
    }
    let until = if !a.command.is_empty() {
        let s = format!("while `{}` runs", a.command.join(" "));
        args.extend(a.command);
        s
    } else if let Some(d) = a.duration {
        format!("for {} (Ctrl-C to stop)", human_duration(d))
    } else {
        "until Ctrl-C".into()
    };
    if !r.dry_run {
        let what = if a.display {
            "system and display"
        } else {
            "system"
        };
        eprintln!("Keeping the {what} awake {until}");
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    r.run("caffeinate", &refs)
}

fn schedule(r: &Runner, s: ScheduleCmd) -> Result<()> {
    let now = chrono::Local::now().naive_local();
    match s {
        ScheduleCmd::List => print!("{}", exec::output("pmset", &["-g", "sched"])?),
        ScheduleCmd::Add { kind, when } => {
            let dt = timeparse::parse_when(&when.join(" "), now)?;
            if dt <= now {
                bail!("date/time is in the past: {dt}");
            }
            let ts = timeparse::pmset_datetime(dt);
            r.run_root("pmset", &["schedule", kind.as_pmset(), &ts])?;
            if !r.dry_run {
                println!(
                    "Scheduled {} at {}",
                    kind.as_pmset(),
                    dt.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
        ScheduleCmd::Cancel { kind, when } => {
            let dt = timeparse::parse_when(&when.join(" "), now)?;
            let ts = timeparse::pmset_datetime(dt);
            r.run_root("pmset", &["schedule", "cancel", kind.as_pmset(), &ts])?;
        }
        ScheduleCmd::Clear { yes } => {
            if r.dry_run || confirm("Cancel all one-time events?", yes)? {
                r.run_root("pmset", &["schedule", "cancelall"])?;
            }
        }
        ScheduleCmd::Repeat { kind, days, time } => {
            let days = timeparse::pmset_weekdays(&days)?;
            let t = timeparse::parse_time(&time)?.format("%H:%M:%S").to_string();
            r.run_root("pmset", &["repeat", kind.as_pmset(), &days, &t])?;
        }
        ScheduleCmd::RepeatCancel => r.run_root("pmset", &["repeat", "cancel"])?,
    }
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
