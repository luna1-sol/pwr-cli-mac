use anyhow::{Context, Result, bail};
use std::io::{self, IsTerminal, Write};
use std::process::Command;

pub struct Runner {
    pub dry_run: bool,
}

impl Runner {
    pub fn run(&self, prog: &str, args: &[&str]) -> Result<()> {
        if self.dry_run {
            println!("+ {}", shell_join(prog, args));
            return Ok(());
        }
        let status = Command::new(prog)
            .args(args)
            .status()
            .with_context(|| format!("failed to start {prog}"))?;
        if !status.success() {
            match status.code() {
                Some(code) => bail!("{} failed with exit code {code}", shell_join(prog, args)),
                None => bail!("{} was terminated by a signal", shell_join(prog, args)),
            }
        }
        Ok(())
    }

    pub fn run_root(&self, prog: &str, args: &[&str]) -> Result<()> {
        if is_root() {
            self.run(prog, args)
        } else {
            let mut full = vec![prog];
            full.extend_from_slice(args);
            self.run("sudo", &full)
        }
    }

    pub fn osascript(&self, script: &str) -> Result<()> {
        self.run("osascript", &["-e", script]).map_err(|e| {
            e.context(
                "AppleScript failed. On first use, allow your terminal to control System Events in \
                 System Settings > Privacy & Security > Automation",
            )
        })
    }
}

pub fn output(prog: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(prog)
        .args(args)
        .output()
        .with_context(|| format!("failed to start {prog}"))?;
    if !out.status.success() {
        bail!(
            "{} failed: {}",
            shell_join(prog, args),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn confirm(message: &str, yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        bail!("cannot ask for confirmation (stdin is not a terminal); pass --yes to proceed");
    }
    print!("{message} [y/N]: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn shell_join(prog: &str, args: &[&str]) -> String {
    std::iter::once(prog)
        .chain(args.iter().copied())
        .map(quote)
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote(s: &str) -> String {
    let safe = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:+=,@%".contains(c));
    if safe {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}
