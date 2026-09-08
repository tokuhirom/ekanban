//! コマンド名で振り分ける表（`docs/DESIGN.md`「コマンドとイベント」）。
//!
//! **名前と引数の形は Tauri 側（`ipc.rs`）と同じ**です。`#[tauri::command]` は
//! Tauri が名前で引くので表が要りませんが、Tauri の外から同じコマンドを呼ぶ側
//! ——開発用のハーネス（`crates/harness`）と、ブラウザ向けの組み立て
//! （`crates/web`、[ADR 0035]）——には要ります。**その表を 1 つにするために
//! ここにあります。** 2 つ持つと、片方にだけコマンドが増えた日に、通っている
//! はずの画面が通りません。
//!
//! ここに書くのは引数の読み取りだけです。判断が入りはじめたら、それは呼ぶ側
//! ごとに違う動きになるということで、[ADR 0021]（偽物のバックエンドを書かない）
//! が守っているものが崩れます。
//!
//! **環境で答えが変わるコマンドはここに入れません。** ファイルの保存先を選ぶ、
//! 場所を開く、URL を開く、グローバルホットキーを割り当てる——どれも「その環境に
//! そういうものがあるか」の話なので、[`invoke`] は `None` を返し、呼ぶ側が
//! 引き受けます。
//!
//! [ADR 0021]: ../../../docs/adr/0021-two-layer-testing-for-the-webview.md
//! [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md

use serde::Deserialize;
use serde_json::Value;

use crate::commands;
use crate::error::{AppError, ErrorKind};
use crate::snapshot::ThemePreference;
use crate::state::AppState;

/// 引数を読んで答えを組み立てる。
///
/// 知らないコマンドと、環境が引き受けるコマンドでは `None` を返します。呼ぶ側は
/// 自分のぶんを先に見て、残りをここに渡してください。
pub fn invoke(command: &str, args: Value, state: &AppState) -> Option<Result<Value, AppError>> {
    match dispatch(command, args, state) {
        Ok(Some(value)) => Some(Ok(value)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    }
}

/// [`invoke`] の中身。
///
/// 「引き受けなかった」を `Ok(None)` で返すのは、**腕の中で `?` を素直に書く
/// ため**です。`Option<Result<_, _>>` のまま組むと、腕ごとに包み直すことに
/// なります。
fn dispatch(command: &str, args: Value, state: &AppState) -> Result<Option<Value>, AppError> {
    let answer = match command {
        "startup_state" => json(commands::startup_state(state)?)?,
        "load_documents" => json(commands::load_documents(state)?)?,
        "save_document" => {
            let a: SaveDocument = read(args)?;
            json(commands::save_document(state, a.document, a.events)?)?
        }
        "create_board" => json(commands::create_board(state, &read::<Name>(args)?.name)?)?,
        "delete_board" => json(commands::delete_board(
            state,
            read::<BoardId>(args)?.board_id,
        )?)?,
        "set_open_board" => json(commands::set_open_board(
            state,
            read::<BoardId>(args)?.board_id,
        )?)?,
        "set_filter_state" => json(commands::set_filter_state(
            state,
            &read::<Filtering>(args)?.filter,
        )?)?,
        "set_sidebar_collapsed" => json(commands::set_sidebar_collapsed(
            state,
            read::<Collapsed>(args)?.collapsed,
        )?)?,
        "set_theme_preference" => json(commands::set_theme_preference(
            state,
            read::<Theme>(args)?.preference,
        )?)?,
        "capture_target" => json(commands::capture_target(state)?)?,
        "set_capture_target" => {
            let a: SetCaptureTarget = read(args)?;
            json(commands::set_capture_target(
                state,
                a.board_id.zip(a.column_id),
            )?)?
        }
        "log_frontend_error" => {
            commands::log_frontend_error(&read::<Message>(args)?.message);
            json(())?
        }
        _ => return Ok(None),
    };
    Ok(Some(answer))
}

/// 呼ぶ側が引き受けられなかったコマンドへの答え。
pub fn unknown_command(command: &str, host: &str) -> AppError {
    AppError::new(
        ErrorKind::BoardIo,
        "知らないコマンドです",
        format!("{command} は {host} に出ていません"),
    )
}

fn json<T: serde::Serialize>(value: T) -> Result<Value, AppError> {
    serde_json::to_value(value).map_err(|error| {
        AppError::new(
            ErrorKind::BoardIo,
            "答えを組み立てられませんでした",
            error.to_string(),
        )
    })
}

fn read<T: for<'de> Deserialize<'de>>(args: Value) -> Result<T, AppError> {
    serde_json::from_value(args).map_err(|error| {
        AppError::new(
            ErrorKind::Validation,
            "引数を読めませんでした",
            error.to_string(),
        )
    })
}

// ---------------------------------------------------------------- 引数の形
//
// 名前は `ipc.rs` の引数名（camelCase で届きます）に合わせてあります。

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoardId {
    board_id: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Name {
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Filtering {
    filter: ekanban_core::store::FilterState,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Collapsed {
    collapsed: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Message {
    message: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveDocument {
    document: commands::BoardDocument,
    events: Vec<ekanban_core::model::CardEvent>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Theme {
    preference: ThemePreference,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetCaptureTarget {
    board_id: Option<ekanban_core::model::BoardId>,
    column_id: Option<ekanban_core::model::ColumnId>,
}
