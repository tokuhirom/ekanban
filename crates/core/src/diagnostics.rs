//! 起動時の致命的なエラーを、どの起動方法でも気づける形で報告する。
//!
//! `.app` から起動すると stderr がどこにも表示されないため、ログファイルに残し、
//! GUI 起動のときはダイアログでも知らせる。

use std::fmt::Write as _;
use std::io::{IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// ログファイルの場所。
pub fn log_path() -> PathBuf {
    crate::paths::log_file()
}

/// stderr が誰にも読まれない状況かどうか。
///
/// `.app` 起動では stderr が捨てられるのでダイアログが要る。ターミナルから実行した
/// ときはメッセージがそのまま見えるので、ダイアログは出さない。
fn stderr_is_invisible() -> bool {
    !std::io::stderr().is_terminal()
}

/// 致命的ではない出来事をログに残す。
///
/// 画面には出さない。起動のたびに必ず起こることを通知に出すと、通知そのものが
/// 読まれなくなるため。
pub fn log(message: &str) {
    append_to_log(message);
}

/// 致命的なエラーを stderr、ログファイル、(GUI 起動なら) ダイアログに出す。
pub fn report_fatal(message: &str) {
    eprintln!("{message}");
    append_to_log(message);

    if stderr_is_invisible() {
        show_dialog(message);
    }
}

fn append_to_log(message: &str) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    let _ = writeln!(file, "{} ekanban: {message}", timestamp());
}

fn show_dialog(message: &str) {
    let body = format!("{message}\n\nログ: {}", log_path().display());
    show_platform_dialog(&body);
}

#[cfg(target_os = "macos")]
fn show_platform_dialog(body: &str) {
    // メッセージは argv で渡し、AppleScript の文字列エスケープを避ける。
    // 無人環境で固まらないよう、一定時間で自動的に閉じる。
    let _ = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg("on run argv")
        .arg("-e")
        .arg(
            "display dialog (item 1 of argv) with title \"ekanban\" \
             buttons {\"OK\"} default button \"OK\" with icon stop \
             giving up after 120",
        )
        .arg("-e")
        .arg("end run")
        .arg("--")
        .arg(body)
        .status();
}

#[cfg(windows)]
fn show_platform_dialog(body: &str) {
    // 環境変数でメッセージを渡し、PowerShell の文字列エスケープを避ける。
    let _ = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Add-Type -AssemblyName System.Windows.Forms; \
             [System.Windows.Forms.MessageBox]::Show($env:EKANBAN_DIALOG_BODY, 'ekanban', \
             'OK', 'Error') | Out-Null",
        ])
        .env("EKANBAN_DIALOG_BODY", body)
        .status();
}

#[cfg(all(unix, not(target_os = "macos")))]
fn show_platform_dialog(body: &str) {
    // デスクトップ環境によって入っているものが違うので、順に試す。
    // どれも無ければログだけが頼りになる。
    let candidates: [(&str, Vec<String>); 3] = [
        (
            "zenity",
            vec![
                "--error".into(),
                "--title=ekanban".into(),
                format!("--text={body}"),
            ],
        ),
        (
            "kdialog",
            vec![
                "--title".into(),
                "ekanban".into(),
                "--error".into(),
                body.into(),
            ],
        ),
        ("xmessage", vec!["-center".into(), body.into()]),
    ];

    for (program, args) in candidates {
        if Command::new(program).args(args).status().is_ok() {
            return;
        }
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` 形式の UTC タイムスタンプ。
fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let days = seconds.div_euclid(86_400);
    let time_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);

    let mut out = String::new();
    let _ = write!(
        out,
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60,
    );
    out
}

/// Howard Hinnant の civil_from_days: 1970-01-01 からの日数を年月日に変換する。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// パニックもログとダイアログに流すようにする。
///
/// `.app` 起動ではパニックメッセージが stderr ごと捨てられ、無言で落ちたように見えるため。
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        report_fatal(&format!("panicked: {info}"));
    }));
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn converts_days_to_civil_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(59), (1970, 3, 1));
        // 2000-02-29: 400 年周期の閏日。
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(20_635), (2026, 7, 1));
    }
}
