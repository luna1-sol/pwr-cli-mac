# pwr-cli

English | [日本語](README.ja.md)

A power management CLI for macOS, written in Rust. It wraps `pmset`, `caffeinate`, and System Events behind a single, easy-to-use command.

## Installation

```bash
cargo install --path .
```

## Usage

| Command | Description |
| --- | --- |
| `pwr status` (`st`) | Power source, battery level, time remaining, cycle count, max capacity, and more. Supports `--json` |
| `pwr sleep` | Sleep now |
| `pwr display-off` | Turn off the display only |
| `pwr shutdown` / `pwr reboot` | Normal shutdown / reboot via System Events (apps can ask to save unsaved documents) |
| `pwr shutdown --force` | Run immediately with `sudo shutdown -h now` |
| `pwr shutdown --in 30m` | Shut down in 30 minutes (cancel with `sudo killall shutdown`) |
| `pwr logout` | Log out |
| `pwr awake [2h] [-d] [-s]` | Prevent sleep. Runs until Ctrl-C if no duration is given. `-d` also keeps the display awake |
| `pwr awake -- cargo build` | Prevent sleep only while a command runs |
| `pwr assertions` | List processes preventing sleep (`--all` for everything) |
| `pwr settings` | Show current power settings |
| `pwr set displaysleep 10 --battery` | Change a power setting (sudo). `--ac` / `--battery`, or both if omitted |
| `pwr low-power on\|off` | Turn Low Power Mode on or off (sudo) |
| `pwr nosleep on\|off` | Disable sleep entirely (sudo). The Mac won't sleep even with the lid closed. This persists across reboots, so turn it `off` when you're done |
| `pwr schedule list` | List scheduled events |
| `pwr schedule add wake 07:30` | Add a one-time event (sudo). Date/time can be `07:30` / `+2h` / `tomorrow 07:30` / `2026-12-24 07:30` |
| `pwr schedule cancel wake 07:30` | Cancel a one-time event |
| `pwr schedule clear` | Cancel all one-time events |
| `pwr schedule repeat wakeorpoweron weekdays 08:00` | Set a weekly repeating event. Days can be `MTWRFSU` / `daily` / `weekdays` / `weekends` |
| `pwr schedule repeat-cancel` | Cancel repeating events |

Event types: `wake` `sleep` `poweron` `shutdown` `reboot` `wakeorpoweron`

Durations: `45s` `30m` `2h` `1h30m` `1d`. A number without a unit is treated as minutes.

Add `-n` / `--dry-run` to any command to print the commands it would run without executing them.

```bash
pwr -n schedule add shutdown +2h
```

## Notes

- `shutdown` / `reboot` / `logout` ask for confirmation first. Use `-y` to skip it.
- The first time you run `shutdown` / `reboot` / `logout`, macOS asks whether your terminal may control System Events (System Settings > Privacy & Security > Automation).
- Changing settings and scheduling events require administrator privileges, so `sudo` is added automatically.

## Scheduled wake and remote access

Whether the display turns on and whether you can connect over ssh after `pwr schedule add wake` depends on the power source and the lid. This is how macOS itself behaves.

| Power | Lid | At the scheduled time | Display | ssh |
| --- | --- | --- | --- | --- |
| AC adapter | Open | Wakes up | On | Reachable |
| AC adapter | Closed | DarkWake (wakes without turning on the display) | Off | Always reachable |
| Battery | Open | Wakes up | On | Reachable |
| Battery | Closed | DarkWake, then goes right back to sleep | Off | Not reachable |

- ssh stays reachable on AC power with the lid closed because "Wake for network access" (`womp`) is `1` on AC power and `0` on battery. Check it with `pmset -g custom`.
- To connect on battery with the lid closed, the only option is to disable sleep with `pwr nosleep on`. Watch out for heat and battery drain, and run `pwr nosleep off` when you're done.
- To check whether the Mac actually slept or woke, look for `Sleep` / `Wake` / `DarkWake` lines in `pmset -g log`.

## Releasing

1. Bump `version` in `Cargo.toml` and commit
2. Tag the same version and push the tag

```bash
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions builds a universal binary and publishes it to Releases. The workflow fails if the tag doesn't match the version in `Cargo.toml`.

## License

[MIT](LICENSE)
