//! `docs/TAURI-MIGRATION.md` §3 のコマンド。
//!
//! **1 つのコマンドが 1 つのモデル操作を呼び、保存し、スナップショットを返す。**
//! 名前は `model.rs` / `Database` のメソッドに揃えてあります。
//!
//! ここに `tauri` は出てきません。`#[tauri::command]` の包みは段階 3 で足し、
//! §10 の開発用ハーネスは同じ関数を HTTP に出します。**偽物のバックエンドを
//! TypeScript で書かない**ための土台なので、この層が Tauri を知らないことは
//! 都合ではなく設計です。

use std::path::{Path, PathBuf};

use chrono::Local;
use ekanban_core::db::{Database, FilterState, WindowBoundsState};
use ekanban_core::model::{
    card_matches_search, parse_due_date, parse_wip_limit, Board, BoardError, BoardId, CardId,
    ChecklistItemDraft, ColumnId, TagId,
};
use ekanban_core::{backup, diagnostics, export};

use crate::error::{AppError, ErrorKind};
use crate::snapshot::{CaptureTarget, Snapshot, StartupState, ThemePreference};
use crate::state::{snapshot_of, AppState};

// ---------------------------------------------------------------- 起動

/// 開くボードと、その付随状態をデータベースから読む。
///
/// 最後に開いていたボードが消えていれば先頭のボードに、絞り込みのタグや
/// キャプチャ先が消えていれば既定に、それぞれ黙って戻します。起動を妨げません。
pub fn load_startup_state(database_path: &Path) -> Result<(AppState, StartupState), AppError> {
    let database = Database::open(database_path).map_err(open_failed)?;
    let boards = database.load_boards().map_err(open_failed)?;
    let board_id = database
        .load_last_board_id()
        .map_err(open_failed)?
        .filter(|board_id| boards.iter().any(|board| board.id == *board_id))
        .or_else(|| boards.first().map(|board| board.id))
        .ok_or_else(|| open_failed(ekanban_core::db::DbError::NoBoard))?;
    let board = database.load_board_by_id(board_id).map_err(open_failed)?;
    database.set_last_board_id(board.id).map_err(open_failed)?;

    let state = AppState::open(database_path, board);
    let startup = startup_state(&state)?;
    Ok((state, startup))
}

/// 開いている盤面と、付随する表示の状態。
///
/// webview が起動のときに 1 回呼びます。`load_startup_state` が起動の入口で
/// 呼ぶのと同じもので、**ウィンドウを開き直すときも同じ経路を通ります**。
/// 閉じている間もクイックキャプチャはカードを足せるので、メモリ上の値を
/// 抱えて使い回さず、そのつどデータベースから読みます（`docs/DESIGN.md`）。
pub fn startup_state(state: &AppState) -> Result<StartupState, AppError> {
    let mut database = state.database().map_err(open_failed)?;

    let mut filter = database.load_filter_state().unwrap_or_default();
    // 絞り込んでいたタグが消えていたら、黙って既定に戻す。起動を妨げない。
    let tag_is_gone = filter
        .tag_id
        .is_some_and(|tag_id| !state.lock().tags.iter().any(|tag| tag.id == tag_id));
    if tag_is_gone {
        filter.tag_id = None;
        database.set_filter_state(&filter).map_err(open_failed)?;
    }

    Ok(StartupState {
        snapshot: state.snapshot()?,
        filter,
        window_bounds: database.load_window_bounds().ok().flatten(),
        theme: ThemePreference::parse(database.load_theme_preference().ok().flatten().as_deref()),
        sidebar_collapsed: database.load_sidebar_collapsed().unwrap_or(false),
        capture_target: read_capture_target(&mut database)?,
        quick_capture_shortcut: database.load_quick_capture_shortcut().unwrap_or(None),
    })
}

fn open_failed(error: ekanban_core::db::DbError) -> AppError {
    AppError::from_db(ErrorKind::BoardIo, "ボードを読めませんでした", &error)
}

// ---------------------------------------------------------------- ボード

pub fn create_board(state: &AppState, name: &str) -> Result<Snapshot, AppError> {
    let mut database = state.database().map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを作れませんでした", &error)
    })?;
    let board = database.create_board(name).map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを作れませんでした", &error)
    })?;
    database.set_last_board_id(board.id).map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを作れませんでした", &error)
    })?;
    let snapshot = snapshot_of(&board, &database).map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボード一覧を読めませんでした", &error)
    })?;
    state.replace(board);
    Ok(snapshot)
}

pub fn rename_board(state: &AppState, name: &str) -> Result<Snapshot, AppError> {
    state
        .mutate(
            "ボードの名前を変えられませんでした",
            |board| board.rename(name),
        )
        .map(|(_, snapshot)| snapshot)
}

/// ボードを消し、残っているボードのどれかを開く。
///
/// 最後の 1 つは消せません（`Database::delete_board` が拒否します）。
pub fn delete_board(state: &AppState, board_id: BoardId) -> Result<Snapshot, AppError> {
    let fail = |error: &ekanban_core::db::DbError| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを削除できませんでした", error)
    };
    let mut database = state.database().map_err(|e| fail(&e))?;
    database.delete_board(board_id).map_err(|e| fail(&e))?;

    let next_id = if state.lock().id == board_id {
        database
            .load_boards()
            .map_err(|e| fail(&e))?
            .first()
            .map(|summary| summary.id)
            .ok_or_else(|| fail(&ekanban_core::db::DbError::NoBoard))?
    } else {
        state.lock().id
    };
    switch_board(state, next_id)
}

pub fn switch_board(state: &AppState, board_id: BoardId) -> Result<Snapshot, AppError> {
    let fail = |error: &ekanban_core::db::DbError| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを開けませんでした", error)
    };
    let database = state.database().map_err(|e| fail(&e))?;
    let board = database.load_board_by_id(board_id).map_err(|e| fail(&e))?;
    database.set_last_board_id(board.id).map_err(|e| fail(&e))?;
    let snapshot = snapshot_of(&board, &database).map_err(|e| fail(&e))?;
    state.replace(board);
    Ok(snapshot)
}

// ---------------------------------------------------------------- カード

const CARD: &str = "カードを操作できませんでした";
const COLUMN: &str = "カラムを操作できませんでした";
const TAG: &str = "タグを操作できませんでした";

/// カードを 1 枚足す。**タイトルが決まってから 1 回だけ呼ばれます。**
///
/// gpui 版は先にカードを足し、タイトルが入るまで保存を保留し、取り下げられたら
/// 引っこめていました。下書きは webview のものになったので（§2）その経路は
/// 消え、「無題のカードを作らない」は**ここで断ればよくなりました**。
///
/// `Board::add_card` はタイトルを見ません（gpui 版が空文字で呼んでいたため）。
/// その規則を保つのはこの層です。
pub fn add_card(
    state: &AppState,
    column_id: ColumnId,
    title: &str,
    description: &str,
) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| {
            if title.trim().is_empty() {
                return Err(BoardError::EmptyCardTitle);
            }
            board.add_card(column_id, title, description)
        })
        .map(|(_, snapshot)| snapshot)
}

/// カードの中身をまとめて書き換える。チェックリストも項目ごと一括で受ける。
///
/// 期限は `"YYYY-MM-DD"` の文字列か空文字で受けます。読めない値は入力欄に返る
/// `Validation` になります（§3）。
#[allow(clippy::too_many_arguments)]
pub fn update_card(
    state: &AppState,
    card_id: CardId,
    title: &str,
    description: &str,
    due_date: &str,
    tag_ids: Vec<TagId>,
    checklist: Vec<ChecklistItemDraft>,
) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| {
            let due_date = parse_due_date(due_date)?;
            board.update_card_details_with_checklist(
                card_id,
                title,
                description,
                due_date,
                tag_ids,
                checklist,
            )
        })
        .map(|(_, snapshot)| snapshot)
}

pub fn move_card(
    state: &AppState,
    card_id: CardId,
    to_column_id: ColumnId,
    to_index: usize,
) -> Result<Snapshot, AppError> {
    state
        .mutate("カードを移動できませんでした", |board| {
            board.move_card(card_id, to_column_id, to_index)
        })
        .map(|(_, snapshot)| snapshot)
}

pub fn copy_card(state: &AppState, card_id: CardId) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| board.copy_card(card_id))
        .map(|(_, snapshot)| snapshot)
}

pub fn delete_card(state: &AppState, card_id: CardId) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| board.delete_card(card_id))
        .map(|(_, snapshot)| snapshot)
}

pub fn archive_card(state: &AppState, card_id: CardId) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| board.archive_card(card_id))
        .map(|(_, snapshot)| snapshot)
}

pub fn restore_card(state: &AppState, card_id: CardId) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| board.restore_card(card_id))
        .map(|(_, snapshot)| snapshot)
}

pub fn set_card_due_date(
    state: &AppState,
    card_id: CardId,
    due_date: &str,
) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| {
            board.set_card_due_date(card_id, parse_due_date(due_date)?)
        })
        .map(|(_, snapshot)| snapshot)
}

pub fn set_card_tags(
    state: &AppState,
    card_id: CardId,
    tag_ids: Vec<TagId>,
) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| board.set_card_tags(card_id, tag_ids))
        .map(|(_, snapshot)| snapshot)
}

// ---------------------------------------------------------------- カラム

pub fn add_column(state: &AppState, name: &str) -> Result<Snapshot, AppError> {
    state
        .mutate(COLUMN, |board| board.add_column(name))
        .map(|(_, snapshot)| snapshot)
}

pub fn rename_column(
    state: &AppState,
    column_id: ColumnId,
    name: &str,
) -> Result<Snapshot, AppError> {
    state
        .mutate(COLUMN, |board| board.rename_column(column_id, name))
        .map(|(_, snapshot)| snapshot)
}

pub fn remove_column(state: &AppState, column_id: ColumnId) -> Result<Snapshot, AppError> {
    state
        .mutate(COLUMN, |board| board.remove_column(column_id))
        .map(|(_, snapshot)| snapshot)
}

pub fn move_column(
    state: &AppState,
    column_id: ColumnId,
    to_index: usize,
) -> Result<Snapshot, AppError> {
    state
        .mutate("カラムを移動できませんでした", |board| {
            board.move_column(column_id, to_index)
        })
        .map(|(_, snapshot)| snapshot)
}

/// WIP 上限を決める。空欄で「上限なし」。
pub fn set_column_wip_limit(
    state: &AppState,
    column_id: ColumnId,
    wip_limit: &str,
) -> Result<Snapshot, AppError> {
    state
        .mutate(COLUMN, |board| {
            board.set_column_wip_limit(column_id, parse_wip_limit(wip_limit)?)
        })
        .map(|(_, snapshot)| snapshot)
}

pub fn sort_column_by_due_date(
    state: &AppState,
    column_id: ColumnId,
) -> Result<Snapshot, AppError> {
    state
        .mutate(COLUMN, |board| board.sort_column_by_due_date(column_id))
        .map(|(_, snapshot)| snapshot)
}

pub fn archive_column(state: &AppState, column_id: ColumnId) -> Result<Snapshot, AppError> {
    state
        .mutate(COLUMN, |board| board.archive_column(column_id))
        .map(|(_, snapshot)| snapshot)
}

// ---------------------------------------------------------------- タグ

pub fn add_tag(state: &AppState, name: &str, color: &str) -> Result<Snapshot, AppError> {
    state
        .mutate(TAG, |board| board.add_tag(name, color))
        .map(|(_, snapshot)| snapshot)
}

pub fn rename_tag(state: &AppState, tag_id: TagId, name: &str) -> Result<Snapshot, AppError> {
    state
        .mutate(TAG, |board| board.rename_tag(tag_id, name))
        .map(|(_, snapshot)| snapshot)
}

pub fn set_tag_color(state: &AppState, tag_id: TagId, color: &str) -> Result<Snapshot, AppError> {
    state
        .mutate(TAG, |board| board.set_tag_color(tag_id, color))
        .map(|(_, snapshot)| snapshot)
}

pub fn remove_tag(state: &AppState, tag_id: TagId) -> Result<Snapshot, AppError> {
    state
        .mutate(TAG, |board| board.remove_tag(tag_id))
        .map(|(_, snapshot)| snapshot)
}

// ---------------------------------------------------------------- 取り消し

pub fn undo(state: &AppState) -> Result<Snapshot, AppError> {
    state
        .mutate("操作を元に戻せませんでした", Board::undo)
        .map(|(_, snapshot)| snapshot)
}

pub fn redo(state: &AppState) -> Result<Snapshot, AppError> {
    state
        .mutate("操作をやり直せませんでした", Board::redo)
        .map(|(_, snapshot)| snapshot)
}

// ---------------------------------------------------------------- 絞り込み

/// 検索語とタグに一致するカードの ID を返す（§5）。
///
/// **判定は Rust に残します。** 全角半角と大文字小文字の正規化を TypeScript で
/// もう一度書くと、2 つの正規化がずれた日にカードが見つからなくなります。
/// 打鍵ごとに呼ばれますが、返るのは ID の配列だけです。
///
/// `#12` はカード番号として読みます（[ADR 0008]）。アーカイブしたカードも
/// 含めて返すので、隠す・減光するの使い分けは呼ぶ側が決めます（[ADR 0010]）。
///
/// [ADR 0008]: ../../../docs/adr/0008-reaching-a-card-by-its-number.md
/// [ADR 0010]: ../../../docs/adr/0010-hiding-instead-of-dimming-in-the-archive.md
pub fn filter_cards(state: &AppState, query: &str, tag_id: Option<TagId>) -> Vec<CardId> {
    let board = state.lock();
    board
        .columns
        .iter()
        .flat_map(|column| column.cards.iter())
        .chain(board.archived_cards.iter())
        // `#12` の読み替えも、全角半角と大文字小文字の正規化も
        // `card_matches_search` の中にある。空の検索語はすべてに一致する。
        .filter(|card| card_matches_search(card, query))
        .filter(|card| tag_id.is_none_or(|tag_id| card.tag_ids.contains(&tag_id)))
        .map(|card| card.id)
        .collect()
}

// ---------------------------------------------------------------- 表示の状態

fn store(
    state: &AppState,
    title: &'static str,
    write: impl FnOnce(&Database) -> Result<(), ekanban_core::db::DbError>,
) -> Result<(), AppError> {
    let database = state
        .database()
        .map_err(|error| AppError::from_db(ErrorKind::Save, title, &error))?;
    write(&database).map_err(|error| AppError::from_db(ErrorKind::Save, title, &error))
}

pub fn set_filter_state(state: &AppState, filter: &FilterState) -> Result<(), AppError> {
    store(
        state,
        "絞り込みを覚えられませんでした",
        |database| database.set_filter_state(filter),
    )
}

pub fn set_theme_preference(state: &AppState, preference: ThemePreference) -> Result<(), AppError> {
    store(
        state,
        "テーマを覚えられませんでした",
        |database| database.set_theme_preference(preference.as_str()),
    )
}

pub fn set_sidebar_collapsed(state: &AppState, collapsed: bool) -> Result<(), AppError> {
    store(
        state,
        "サイドバーの状態を覚えられませんでした",
        |database| database.set_sidebar_collapsed(collapsed),
    )
}

pub fn set_window_bounds(state: &AppState, bounds: WindowBoundsState) -> Result<(), AppError> {
    store(
        state,
        "ウィンドウの位置を覚えられませんでした",
        |database| database.set_window_bounds(bounds),
    )
}

// ---------------------------------------------------------------- ファイル

/// 書き出す形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExportFormat {
    Json,
    Markdown,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
        }
    }
}

/// 保存ダイアログに出す既定のファイル名。
pub fn suggested_export_name(state: &AppState, format: ExportFormat) -> String {
    export::suggested_export_name(&state.lock().name, format.extension())
}

/// 開いているボードをファイルに書き出す。書けたパスを返す。
///
/// 行き先を選ぶのは呼ぶ側（OS のネイティブな保存ダイアログ、§9）です。ここは
/// 中身を作って書くだけにして、ダイアログの都合をコマンドの層に持ち込みません。
pub fn export_board(
    state: &AppState,
    format: ExportFormat,
    destination: &Path,
) -> Result<PathBuf, AppError> {
    let contents = match format {
        ExportFormat::Json => {
            let database = state.database().map_err(|error| {
                AppError::from_db(ErrorKind::Export, "書き出せませんでした", &error)
            })?;
            let board = state.lock();
            database.export_board_json(&board).map_err(|error| {
                AppError::from_db(ErrorKind::Export, "書き出せませんでした", &error)
            })?
        }
        ExportFormat::Markdown => export::render_board_markdown(&state.lock()),
    };
    std::fs::write(destination, contents).map_err(|error| {
        AppError::new(
            ErrorKind::Export,
            "書き出せませんでした",
            format!("{} に書けませんでした: {error}", destination.display()),
        )
    })?;
    Ok(destination.to_path_buf())
}

/// データベースの控えを、選んだ場所に取る。書けたパスを返す。
pub fn backup_database(state: &AppState, destination: &Path) -> Result<PathBuf, AppError> {
    let database = state.database().map_err(|error| {
        AppError::from_db(ErrorKind::Export, "控えを保存できませんでした", &error)
    })?;
    database.backup_to(destination).map_err(|error| {
        AppError::from_db(ErrorKind::Export, "控えを保存できませんでした", &error)
    })?;
    Ok(destination.to_path_buf())
}

/// データベースそのものの場所。
pub fn database_location(state: &AppState) -> PathBuf {
    state.database_path().to_path_buf()
}

/// 「場所を開く」で開く先。実際に開くのは呼ぶ側（`tauri-plugin-opener`、§9）。
pub fn reveal_database(state: &AppState) -> PathBuf {
    state.database_path().to_path_buf()
}

/// 日ごとの控えが溜まるディレクトリ。
pub fn reveal_backups(state: &AppState) -> PathBuf {
    backup::directory(state.database_path())
}

// ---------------------------------------------------------------- キャプチャ

fn read_capture_target(database: &mut Database) -> Result<Option<CaptureTarget>, AppError> {
    // キャプチャ先のボードやカラムが消えていたら、黙って既定に戻す。
    // 絞り込みの復元と同じ扱いで、起動を妨げない。
    let Some((board_id, column_id)) = database.load_capture_target().unwrap_or(None) else {
        return Ok(None);
    };
    let column_name = database
        .load_column_name(board_id, column_id)
        .unwrap_or(None);
    let board_name = database
        .load_boards()
        .unwrap_or_default()
        .into_iter()
        .find(|summary| summary.id == board_id)
        .map(|summary| summary.name);
    match (board_name, column_name) {
        (Some(board_name), Some(column_name)) => Ok(Some(CaptureTarget {
            board_id,
            column_id,
            board_name,
            column_name,
        })),
        _ => {
            database.set_capture_target(None).map_err(|error| {
                AppError::from_db(ErrorKind::Save, "キャプチャ先を消せませんでした", &error)
            })?;
            Ok(None)
        }
    }
}

/// クイックキャプチャからカードを 1 枚足す。
///
/// **ボードと同じ保存経路に乗せます**（§9）——カラムの末尾に足し、Undo の対象に
/// なり、`created` が 1 件積まれます。キャプチャ先が開いているボードと違うときは、
/// そちらを読んで書き、開いている盤面はそのままにします。
pub fn capture_card(state: &AppState, title: &str) -> Result<Snapshot, AppError> {
    const FAILED: &str = "カードを追加できませんでした";
    let mut database = state
        .database()
        .map_err(|error| AppError::from_db(ErrorKind::Save, FAILED, &error))?;
    let target = read_capture_target(&mut database)?.ok_or_else(|| {
        AppError::new(
            ErrorKind::Save,
            FAILED,
            "カードの追加先が決まっていません。ボードのメニューから選んでください",
        )
    })?;

    if title.trim().is_empty() {
        return Err(AppError::from_board(FAILED, &BoardError::EmptyCardTitle));
    }

    if state.lock().id == target.board_id {
        return state
            .mutate(FAILED, |board| board.add_card(target.column_id, title, ""))
            .map(|(_, snapshot)| snapshot);
    }

    let mut other = database
        .load_board_by_id(target.board_id)
        .map_err(|error| AppError::from_db(ErrorKind::Save, FAILED, &error))?;
    other
        .add_card(target.column_id, title, "")
        .map_err(|error| AppError::from_board(FAILED, &error))?;
    database
        .save_board(&mut other)
        .map_err(|error| AppError::from_save(&error))?;
    state.snapshot()
}

pub fn set_capture_target(
    state: &AppState,
    target: Option<(BoardId, ColumnId)>,
) -> Result<(), AppError> {
    store(
        state,
        "カードの追加先を覚えられませんでした",
        |database| database.set_capture_target(target),
    )
}

/// 割り当てを覚える。**登録できなかった割り当ては保存しません**（§9）ので、
/// 呼ぶ側が登録に成功してからここを呼びます。
pub fn set_quick_capture_shortcut(
    state: &AppState,
    shortcut: Option<&str>,
) -> Result<(), AppError> {
    store(
        state,
        "割り当てを覚えられませんでした",
        |database| database.set_quick_capture_shortcut(shortcut),
    )
}

// ---------------------------------------------------------------- 記録

/// webview の未捕捉例外を、Rust 側と同じログに落とす（§9）。
///
/// webview の失敗が黙って消えると、原因を追う手段がなくなります。
pub fn log_frontend_error(message: &str) {
    diagnostics::log(&format!("webview: {message}"));
}

/// その日ぶんの控えを 1 つ残す。起動のときに別スレッドから呼びます。
///
/// 失敗しても起動は止めません（`docs/DESIGN.md`）。取るのは起動時で、終了時では
/// ない——終了時に取ると、壊した状態のほうを保存することになります。
pub fn run_daily_backup(database_path: &Path) {
    if let Err(error) = backup::run_daily(database_path, Local::now().date_naive()) {
        diagnostics::log(&format!(
            "failed to back up {}: {error}",
            database_path.display()
        ));
    }
}
