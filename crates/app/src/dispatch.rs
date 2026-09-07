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

use crate::commands::{self, ExportFormat};
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
        "snapshot" => json(state.snapshot()?)?,
        "switch_board" => json(commands::switch_board(
            state,
            read::<BoardId>(args)?.board_id,
        )?)?,
        "create_board" => json(commands::create_board(state, &read::<Name>(args)?.name)?)?,
        "rename_board" => json(commands::rename_board(state, &read::<Name>(args)?.name)?)?,
        "delete_board" => json(commands::delete_board(
            state,
            read::<BoardId>(args)?.board_id,
        )?)?,
        "add_card" => {
            let a: AddCard = read(args)?;
            json(commands::add_card(
                state,
                a.column_id,
                &a.title,
                &a.description,
                &a.due_date,
                a.tag_ids,
                a.checklist,
            )?)?
        }
        "update_card" => {
            let a: UpdateCard = read(args)?;
            json(commands::update_card(
                state,
                a.card_id,
                &a.title,
                &a.description,
                &a.due_date,
                a.tag_ids,
                a.checklist,
            )?)?
        }
        "copy_card" => json(commands::copy_card(state, read::<CardId>(args)?.card_id)?)?,
        "delete_card" => json(commands::delete_card(state, read::<CardId>(args)?.card_id)?)?,
        "archive_card" => json(commands::archive_card(
            state,
            read::<CardId>(args)?.card_id,
        )?)?,
        "restore_card" => json(commands::restore_card(
            state,
            read::<CardId>(args)?.card_id,
        )?)?,
        "set_card_tags" => {
            let a: CardTags = read(args)?;
            json(commands::set_card_tags(state, a.card_id, a.tag_ids)?)?
        }
        "set_card_due_date" => {
            let a: CardDueDate = read(args)?;
            json(commands::set_card_due_date(state, a.card_id, &a.due_date)?)?
        }
        "add_column" => json(commands::add_column(state, &read::<Name>(args)?.name)?)?,
        "rename_column" => {
            let a: ColumnName = read(args)?;
            json(commands::rename_column(state, a.column_id, &a.name)?)?
        }
        "remove_column" => json(commands::remove_column(
            state,
            read::<ColumnId>(args)?.column_id,
        )?)?,
        "archive_column" => json(commands::archive_column(
            state,
            read::<ColumnId>(args)?.column_id,
        )?)?,
        "add_tag" => {
            let a: AddTag = read(args)?;
            json(commands::add_tag(state, &a.name, &a.color)?)?
        }
        "rename_tag" => {
            let a: TagName = read(args)?;
            json(commands::rename_tag(state, a.tag_id, &a.name)?)?
        }
        "set_tag_color" => {
            let a: TagColor = read(args)?;
            json(commands::set_tag_color(state, a.tag_id, &a.color)?)?
        }
        "remove_tag" => json(commands::remove_tag(state, read::<TagId>(args)?.tag_id)?)?,
        "move_card" => {
            let a: MoveCard = read(args)?;
            json(commands::move_card(
                state,
                a.card_id,
                a.to_column_id,
                a.to_index,
            )?)?
        }
        "move_column" => {
            let a: MoveColumn = read(args)?;
            json(commands::move_column(state, a.column_id, a.to_index)?)?
        }
        "filter_cards" => {
            let a: Filter = read(args)?;
            json(commands::filter_cards(state, &a.query, a.tag_id))?
        }
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
        "undo" => json(commands::undo(state)?)?,
        "redo" => json(commands::redo(state)?)?,
        "suggested_export_name" => json(commands::suggested_export_name(
            state,
            read::<Format>(args)?.format,
        ))?,
        "due_date_preview" => json(commands::due_date_preview(&read::<DueText>(args)?.value))?,
        "capture_target" => json(commands::capture_target(state)?)?,
        "set_capture_column" => json(commands::set_capture_column(
            state,
            read::<CaptureColumn>(args)?.column_id,
        )?)?,
        "capture_card" => json(commands::capture_card(state, &read::<Title>(args)?.title)?)?,
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
struct AddCard {
    column_id: i64,
    title: String,
    description: String,
    due_date: String,
    tag_ids: Vec<i64>,
    checklist: Vec<ekanban_core::model::ChecklistItemDraft>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCard {
    card_id: i64,
    title: String,
    description: String,
    due_date: String,
    tag_ids: Vec<i64>,
    checklist: Vec<ekanban_core::model::ChecklistItemDraft>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardId {
    card_id: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardTags {
    card_id: i64,
    tag_ids: Vec<i64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CardDueDate {
    card_id: i64,
    due_date: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColumnId {
    column_id: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColumnName {
    column_id: i64,
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddTag {
    name: String,
    color: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagName {
    tag_id: i64,
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagColor {
    tag_id: i64,
    color: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TagId {
    tag_id: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveCard {
    card_id: i64,
    to_column_id: i64,
    to_index: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveColumn {
    column_id: i64,
    to_index: usize,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Filter {
    query: String,
    tag_id: Option<i64>,
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
struct Theme {
    preference: ThemePreference,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Format {
    format: ExportFormat,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DueText {
    value: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureColumn {
    column_id: Option<ekanban_core::model::ColumnId>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Title {
    title: String,
}
