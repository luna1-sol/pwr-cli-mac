# pwr-cli

macOS の電源操作を行う Rust 製 CLI。`pmset` / `caffeinate` / System Events をまとめて扱いやすくしたラッパーです。

## インストール

```bash
cargo install --path .
```

## 使い方

| コマンド | 内容 |
| --- | --- |
| `pwr status` (`st`) | 電源・バッテリー残量・残り時間・充放電回数・最大容量など。`--json` 対応 |
| `pwr sleep` | 今すぐスリープ |
| `pwr display-off` | ディスプレイだけスリープ |
| `pwr shutdown` / `pwr reboot` | System Events 経由で通常のシャットダウン・再起動 (未保存の書類は確認される) |
| `pwr shutdown --force` | `sudo shutdown -h now` で即時実行 |
| `pwr shutdown --in 30m` | 30 分後にシャットダウン (`sudo killall shutdown` で取り消し) |
| `pwr logout` | ログアウト |
| `pwr awake [2h] [-d] [-s]` | スリープ防止。時間省略時は Ctrl-C まで。`-d` でディスプレイも |
| `pwr awake -- cargo build` | コマンドの実行中だけスリープ防止 |
| `pwr assertions` | スリープを妨げているプロセスの一覧 (`--all` で全て) |
| `pwr settings` | 現在の電源設定 |
| `pwr set displaysleep 10 --battery` | 電源設定の変更 (sudo)。`--ac` / `--battery` / 省略で両方 |
| `pwr low-power on\|off` | 低電力モードの切り替え (sudo) |
| `pwr schedule list` | 予約一覧 |
| `pwr schedule add wake 07:30` | 単発予約 (sudo)。日時は `07:30` / `+2h` / `tomorrow 07:30` / `2026-12-24 07:30` |
| `pwr schedule cancel wake 07:30` | 単発予約の取り消し |
| `pwr schedule clear` | 単発予約をすべて取り消し |
| `pwr schedule repeat wakeorpoweron weekdays 08:00` | 毎週の繰り返し予約。曜日は `MTWRFSU` / `daily` / `weekdays` / `weekends` |
| `pwr schedule repeat-cancel` | 繰り返し予約の取り消し |

予約の種類: `wake` `sleep` `poweron` `shutdown` `reboot` `wakeorpoweron`

時間指定は `45s` `30m` `2h` `1h30m` `1d`。単位を省略すると分として扱います。

すべてのコマンドに `-n` / `--dry-run` を付けると、実行せずに実行予定のコマンドだけを表示します。

```bash
pwr -n schedule add shutdown +2h
```

## 補足

- `shutdown` / `reboot` / `logout` は実行前に確認します。`-y` で確認をスキップできます。
- 初回の `shutdown` / `reboot` / `logout` では、ターミナルが System Events を操作する許可を求められます (システム設定 > プライバシーとセキュリティ > オートメーション)。
- 設定変更と予約には管理者権限が必要なので、自動で `sudo` を付けて実行します。
