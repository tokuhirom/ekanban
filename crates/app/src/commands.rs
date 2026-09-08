//! `docs/DESIGN.md`「コマンドとイベント」のコマンド。
//!
//! **1 つのコマンドが 1 つのモデル操作を呼び、保存し、スナップショットを返す。**
//! 名前は `model.rs` / `Database` のメソッドに揃えてあります。
//!
//! ここに `tauri` は出てきません。`#[tauri::command]` の包みは `ipc.rs` にあり、
//! 開発用のハーネスは同じ関数を HTTP に出します。**偽物のバックエンドを
//! TypeScript で書かない**ための土台なので、この層が Tauri を知らないことは
//! 都合ではなく設計です。

// ファイルの読み書きは殻の側だけ（書き出し、控え、場所を開く、[ADR 0036]）。
#[cfg(feature = "shell")]
use std::path::{Path, PathBuf};

// `shell` の外では使いません（日次バックアップだけが今日を要る）。素の
// `use` にすると、ブラウザ向けの組み立てで未使用の警告が出ます。
#[cfg(feature = "shell")]
use chrono::Local;
use ekanban_core::diagnostics;
use ekanban_core::model::{
    parse_stored_due_date, Board, BoardError, BoardId, CardId, ChecklistItemDraft, ColumnId, TagId,
};
use ekanban_core::store::{FilterState, Store, StoreError, WindowBoundsState};

#[cfg(feature = "shell")]
use ekanban_core::backup;

use crate::error::{AppError, ErrorKind};
use crate::snapshot::{CaptureTarget, Platform, Snapshot, StartupState, ThemePreference};
use crate::state::{snapshot_of, AppState, Source};

// ---------------------------------------------------------------- 起動

/// 開くボードと、その付随状態をデータベースから読む。
///
/// 最後に開いていたボードが消えていれば先頭のボードに、絞り込みのタグや
/// キャプチャ先が消えていれば既定に、それぞれ黙って戻します。起動を妨げません。
pub fn load_startup_state(source: Source) -> Result<(AppState, StartupState), AppError> {
    let mut store = source.open().map_err(open_failed)?;
    // 何も入っていなければ最初の盤面を蒔く。SQLite は開いた時点で蒔いてある。
    store.seed_if_empty().map_err(open_failed)?;
    let boards = store.load_boards().map_err(open_failed)?;
    let board_id = store
        .load_last_board_id()
        .map_err(open_failed)?
        .filter(|board_id| boards.iter().any(|board| board.id == *board_id))
        .or_else(|| boards.first().map(|board| board.id))
        .ok_or_else(|| open_failed(StoreError::NoBoard))?;
    let board = store.load_board_by_id(board_id).map_err(open_failed)?;
    store.set_last_board_id(board.id).map_err(open_failed)?;

    drop(store);
    let state = AppState::open(source, board);
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
    let mut store = state.store().map_err(open_failed)?;

    let mut filter = store.load_filter_state().unwrap_or_default();
    // 絞り込んでいたタグが消えていたら、黙って既定に戻す。起動を妨げない。
    let tag_is_gone = filter
        .tag_id
        .is_some_and(|tag_id| !state.lock().tags.iter().any(|tag| tag.id == tag_id));
    if tag_is_gone {
        filter.tag_id = None;
        store.set_filter_state(&filter).map_err(open_failed)?;
    }

    // **開いてある置き場所からスナップショットを組みます。** `state.snapshot()`
    // を呼ぶと、ここで持っている置き場所をもう 1 つ開くことになります
    // （`AppState::store` の注意書き）。
    let snapshot = {
        let board = state.lock();
        snapshot_of(&board, &store).map_err(open_failed)?
    };

    Ok(StartupState {
        snapshot,
        platform: Platform::current(),
        filter,
        window_bounds: store.load_window_bounds().ok().flatten(),
        theme: ThemePreference::parse(store.load_theme_preference().ok().flatten().as_deref()),
        sidebar_collapsed: store.load_sidebar_collapsed().unwrap_or(false),
        capture_target: read_capture_target(&mut store)?,
        quick_capture_shortcut: store.load_quick_capture_shortcut().unwrap_or(None),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database_path: state.source().place(),
    })
}

fn open_failed(error: StoreError) -> AppError {
    AppError::from_db(ErrorKind::BoardIo, "ボードを読めませんでした", &error)
}

// ---------------------------------------------------------------- ボード

pub fn create_board(state: &AppState, name: &str) -> Result<Snapshot, AppError> {
    let mut store = state.store().map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを作れませんでした", &error)
    })?;
    let board = store.create_board(name).map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを作れませんでした", &error)
    })?;
    store.set_last_board_id(board.id).map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを作れませんでした", &error)
    })?;
    let snapshot = snapshot_of(&board, &store).map_err(|error| {
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
    let fail = |error: &StoreError| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを削除できませんでした", error)
    };
    let mut store = state.store().map_err(|e| fail(&e))?;
    store.delete_board(board_id).map_err(|e| fail(&e))?;

    let next_id = if state.lock().id == board_id {
        store
            .load_boards()
            .map_err(|e| fail(&e))?
            .first()
            .map(|summary| summary.id)
            .ok_or_else(|| fail(&StoreError::NoBoard))?
    } else {
        state.lock().id
    };
    switch_board(state, next_id)
}

pub fn switch_board(state: &AppState, board_id: BoardId) -> Result<Snapshot, AppError> {
    let fail = |error: &StoreError| {
        AppError::from_db(ErrorKind::BoardIo, "ボードを開けませんでした", error)
    };
    let mut store = state.store().map_err(|e| fail(&e))?;
    let board = store.load_board_by_id(board_id).map_err(|e| fail(&e))?;
    store.set_last_board_id(board.id).map_err(|e| fail(&e))?;
    let snapshot = snapshot_of(&board, &store).map_err(|e| fail(&e))?;
    state.replace(board);
    Ok(snapshot)
}

// ---------------------------------------------------------------- カード

const CARD: &str = "カードを操作できませんでした";
const COLUMN: &str = "カラムを操作できませんでした";
const TAG: &str = "タグを操作できませんでした";

/// カードを 1 枚足す。**タイトルが決まってから 1 回だけ呼ばれます。**
///
/// 下書きは webview が持つので（`docs/DESIGN.md`「状態の持ち主」）、保存を
/// 押すまで盤面には何も届きません。だから「無題のカードを作らない」は
/// **ここで断るだけで守れます**——足してから引っこめる経路がありません。
///
/// `Board::add_card_with_details` はタイトルを見ないので、その規則を保つのはこの層です。
///
/// 受け取るのは `update_card` と同じ下書き一式です（#127）。期限・タグ・
/// チェックリストを付けてから足すので、足したあとに開き直す往復が要りません。
/// **モデルを呼ぶのは 1 回だけ**なので、Undo に積まれるのも 1 件です
/// （`docs/DESIGN.md`「コマンドとイベント」）。
pub fn add_card(
    state: &AppState,
    column_id: ColumnId,
    title: &str,
    description: &str,
    due_date: &str,
    tag_ids: Vec<TagId>,
    checklist: Vec<ChecklistItemDraft>,
) -> Result<Snapshot, AppError> {
    state
        .mutate(CARD, |board| {
            if title.trim().is_empty() {
                return Err(BoardError::EmptyCardTitle);
            }
            let due_date = parse_stored_due_date(due_date)?;
            board.add_card_with_details(column_id, title, description, due_date, tag_ids, checklist)
        })
        .map(|(_, snapshot)| snapshot)
}

/// カードの中身をまとめて書き換える。チェックリストも項目ごと一括で受ける。
///
/// 期限は `"YYYY-MM-DD"` の文字列か空文字で受けます。読めない値は入力欄に返る
/// `Validation` になります（`docs/DESIGN.md`「コマンドとイベント」）。
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
            let due_date = parse_stored_due_date(due_date)?;
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
            board.set_card_due_date(card_id, parse_stored_due_date(due_date)?)
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

/// 終わったものの置き場かどうかを切り替える（[ADR 0038]）。
///
/// **何本立てても構いません。** 「完了」と「キャンセル済み」を並べて両方立てるのが、
/// ボードではなくカラムの属性にした理由です。
///
/// [ADR 0038]: ../../../../docs/adr/0038-a-column-that-means-done.md
pub fn set_column_done(
    state: &AppState,
    column_id: ColumnId,
    done: bool,
) -> Result<Snapshot, AppError> {
    state
        .mutate(COLUMN, |board| board.set_column_done(column_id, done))
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

// ---------------------------------------------------------------- 表示の状態

fn store(
    state: &AppState,
    title: &'static str,
    write: impl FnOnce(&mut Store<'_>) -> Result<(), StoreError>,
) -> Result<(), AppError> {
    let mut store = state
        .store()
        .map_err(|error| AppError::from_db(ErrorKind::Save, title, &error))?;
    write(&mut store).map_err(|error| AppError::from_db(ErrorKind::Save, title, &error))
}

pub fn set_filter_state(state: &AppState, filter: &FilterState) -> Result<(), AppError> {
    store(
        state,
        "絞り込みを覚えられませんでした",
        |store| store.set_filter_state(filter),
    )
}

pub fn set_theme_preference(state: &AppState, preference: ThemePreference) -> Result<(), AppError> {
    store(
        state,
        "テーマを覚えられませんでした",
        |store| store.set_theme_preference(preference.as_str()),
    )
}

pub fn set_sidebar_collapsed(state: &AppState, collapsed: bool) -> Result<(), AppError> {
    store(
        state,
        "サイドバーの状態を覚えられませんでした",
        |store| store.set_sidebar_collapsed(collapsed),
    )
}

pub fn set_window_bounds(state: &AppState, bounds: WindowBoundsState) -> Result<(), AppError> {
    store(
        state,
        "ウィンドウの位置を覚えられませんでした",
        |store| store.set_window_bounds(bounds),
    )
}

// ---------------------------------------------------------------- ファイル

/// 選ばれたパスに拡張子を補う。**書き出す先があるのは殻の側だけ**（[ADR 0036]）。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
#[cfg(feature = "shell")]
///
/// 保存ダイアログで名前を打ち替えると、拡張子ごと消えることがあります。
/// 拡張子の無いファイルを書くと、次に開くときに何のファイルか分かりません。
/// **すでに何か付いているものは触りません**——`board.json.txt` を選んだ人の
/// 意図を、こちらで書き換えないためです。
fn with_extension(destination: &Path, extension: &str) -> PathBuf {
    if destination.extension().is_none() {
        destination.with_extension(extension)
    } else {
        destination.to_path_buf()
    }
}

/// 開いているボードを JSON にする。**まだ書きません。**
///
/// 書く先が無い環境があるので分けてあります（ブラウザ、[ADR 0035]）。そこでは
/// この文字列がそのままページへ渡り、ダウンロードになります。
///
/// **組み立てるのがここなのは、置いてある形の写しだから**です（[ADR 0045]）。
/// カードの履歴（`card_events`）まで入り、それは置き場所にしかありません。
/// 人が読む Markdown のほうは webview が組み立てます。
///
/// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
/// [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md
pub fn export_board_json_contents(state: &AppState) -> Result<String, AppError> {
    let store = state
        .store()
        .map_err(|error| AppError::from_db(ErrorKind::Export, "書き出せませんでした", &error))?;
    let board = state.lock();
    store
        .export_board_json(&board)
        .map_err(|error| AppError::from_db(ErrorKind::Export, "書き出せませんでした", &error))
}

/// 開いているボードを JSON のファイルに書き出す。書けたパスを返す。
///
/// **書く先があるのは殻の側だけ**です。ブラウザには書き込めるファイルシステム
/// が無いので、そちらは `export_board_json_contents` の文字列をダウンロードに
/// します（[ADR 0035]）。
///
/// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
#[cfg(feature = "shell")]
pub fn export_board_json(state: &AppState, destination: &Path) -> Result<PathBuf, AppError> {
    write_text_file(destination, "json", &export_board_json_contents(state)?)
}

/// 組み立てられた中身を、選ばれた場所に書く。書けたパスを返す。
///
/// **中身は受け取るだけ**です（[ADR 0045]）。行き先を選ぶのは OS の保存
/// ダイアログ、報せるのはアプリの中のダイアログで（`docs/DESIGN.md`「アプリが
/// 伝えること」）、ここはその間の「書く」だけを引き受けます。
///
/// [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md
#[cfg(feature = "shell")]
pub fn write_text_file(
    destination: &Path,
    extension: &str,
    contents: &str,
) -> Result<PathBuf, AppError> {
    let destination = &with_extension(destination, extension);
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
///
/// **いま使っているファイルそのものは断ります。** `backup_to` は上書きで開くので、
/// 同じパスを渡すと控えを取ったつもりで元のファイルを触ることになります。
///
/// **殻の側だけ**です。控えは SQLite のファイルを写すもので、置き場所の口
/// （`store::Store`）には無い操作です（[ADR 0036]）。ブラウザ版では
/// 「データベースをコピー…」を灰色にしてあります。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
#[cfg(feature = "shell")]
pub fn backup_database(state: &AppState, destination: &Path) -> Result<PathBuf, AppError> {
    let destination = &with_extension(destination, "sqlite3");
    if destination == state.database_path() {
        return Err(AppError::new(
            ErrorKind::Export,
            "控えを保存できませんでした",
            "控えの保存先には、いま使っているデータベースとは別のファイルを指定してください",
        ));
    }
    let database = ekanban_core::db::Database::open(state.database_path()).map_err(|error| {
        AppError::from_db(ErrorKind::Export, "控えを保存できませんでした", &error)
    })?;
    database.backup_to(destination).map_err(|error| {
        AppError::from_db(ErrorKind::Export, "控えを保存できませんでした", &error)
    })?;
    Ok(destination.to_path_buf())
}

/// データベースそのものの場所。
#[cfg(feature = "shell")]
pub fn database_location(state: &AppState) -> PathBuf {
    state.database_path().to_path_buf()
}

/// 「場所を開く」で開く先。実際に開くのは呼ぶ側（`tauri-plugin-opener`、`docs/DESIGN.md`「アプリが伝えること」）。
#[cfg(feature = "shell")]
pub fn reveal_database(state: &AppState) -> PathBuf {
    state.database_path().to_path_buf()
}

/// 日ごとの控えが溜まるディレクトリ。
#[cfg_attr(not(feature = "shell"), allow(dead_code))]
#[cfg(feature = "shell")]
///
/// まだ 1 つも取れていないうちに押されることがあります。開く先が無いだけなので
/// `None` を返し、呼ぶ側は黙って何もしません（拒否は何も言わない、`docs/DESIGN.md`）。
pub fn reveal_backups(state: &AppState) -> Option<PathBuf> {
    let directory = backup::directory(state.database_path());
    directory.is_dir().then_some(directory)
}

// ---------------------------------------------------------------- 期限の下読み

// ---------------------------------------------------------------- 説明のリンク

/// 開いてよい URL か。開けるなら、そのまま返す。
///
/// 拾うのは `http(s)://` だけという [ADR 0002] の決めごとを、**開く側でも
/// 確かめます**。説明はユーザーが打った文字列なので、`file://` や
/// `javascript:` を混ぜられる場所です。
///
/// [ADR 0002]: ../../../docs/adr/0002-links-inside-the-description-field.md
pub fn openable_url(url: &str) -> Option<&str> {
    (url.starts_with("https://") || url.starts_with("http://")).then_some(url)
}

// ---------------------------------------------------------------- キャプチャ

fn read_capture_target(store: &mut Store<'_>) -> Result<Option<CaptureTarget>, AppError> {
    // キャプチャ先のボードやカラムが消えていたら、黙って既定に戻す。
    // 絞り込みの復元と同じ扱いで、起動を妨げない。
    let Some((board_id, column_id)) = store.load_capture_target().unwrap_or(None) else {
        return Ok(None);
    };
    let column_name = store.load_column_name(board_id, column_id).unwrap_or(None);
    let board_name = store
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
            store.set_capture_target(None).map_err(|error| {
                AppError::from_db(ErrorKind::Save, "キャプチャ先を消せませんでした", &error)
            })?;
            Ok(None)
        }
    }
}

/// 設定が無いときの既定の入れ先。**先頭のボードの先頭カラム**（#117、[ADR 0028]）。
///
/// 開いているボードから決めていたころは、ボードを切り替えるだけで入れ先が動いて
/// いました。「キャプチャ先はアプリ全体で 1 つ」（`docs/DESIGN.md`「クイック
/// キャプチャ」）は、設定していないときも同じでなければ成り立ちません。
///
/// 先頭のボードは `load_boards`（`ORDER BY boards.id`）の 1 つめで、サイドバーの
/// 一番上と同じです。そのボードにカラムが 1 本も無ければ `None` です。
///
/// [ADR 0028]: ../../../docs/adr/0028-a-single-default-quick-capture-target.md
fn default_capture_target(store: &Store<'_>) -> Result<Option<CaptureTarget>, AppError> {
    let Some(first) = store.load_boards().unwrap_or_default().into_iter().next() else {
        return Ok(None);
    };
    let board = store.load_board_by_id(first.id).map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "キャプチャ先を読めませんでした", &error)
    })?;
    Ok(board.columns.first().map(|column| CaptureTarget {
        board_id: board.id,
        column_id: column.id,
        board_name: board.name.clone(),
        column_name: column.name.clone(),
    }))
}

/// いまのキャプチャ先。設定が無い・消えているときは既定に落とす。
///
/// 決まっていないから足せない、にはしません——キャプチャは 1 行を放り込むための
/// もので、そこで設定を求めると用が足りません。
///
/// 設定が指していたカラムが消えていたら、黙って設定を消して既定に戻します。
/// 次のキャプチャを失敗させないためです。
pub fn capture_target(state: &AppState) -> Result<Option<CaptureTarget>, AppError> {
    let mut store = state.store().map_err(|error| {
        AppError::from_db(ErrorKind::BoardIo, "キャプチャ先を読めませんでした", &error)
    })?;
    if let Some(target) = read_capture_target(&mut store)? {
        return Ok(Some(target));
    }
    default_capture_target(&store)
}

/// クイックキャプチャからカードを 1 枚足す。
///
/// **ボードと同じ保存経路に乗せます**（`docs/DESIGN.md`「クイックキャプチャ」）——カラムの末尾に足し、Undo の対象に
/// なり、`created` が 1 件積まれます。キャプチャ先が開いているボードと違うときは、
/// そちらを読んで書き、開いている盤面はそのままにします。
pub fn capture_card(state: &AppState, title: &str) -> Result<Snapshot, AppError> {
    const FAILED: &str = "カードを追加できませんでした";
    let mut store = state
        .store()
        .map_err(|error| AppError::from_db(ErrorKind::Save, FAILED, &error))?;
    let stored = read_capture_target(&mut store)?;
    // 設定が無ければ既定（先頭のボードの先頭カラム）へ。
    let target = match stored {
        Some(target) => Some(target),
        None => default_capture_target(&store)?,
    }
    .ok_or_else(|| {
        AppError::new(
            ErrorKind::Save,
            FAILED,
            "カードの追加先が決まっていません。ボードにカラムを 1 つ足してください",
        )
    })?;

    if title.trim().is_empty() {
        return Err(AppError::from_board(FAILED, &BoardError::EmptyCardTitle));
    }

    if state.lock().id == target.board_id {
        // **置き場所を放してから盤面の経路に渡します。** `mutate` は自分で
        // 開き直すので、持ったままだと 2 度開くことになります
        // （`AppState::store` の注意書き）。
        drop(store);
        return state
            .mutate(FAILED, |board| board.add_card(target.column_id, title, ""))
            .map(|(_, snapshot)| snapshot);
    }

    let mut other = store
        .load_board_by_id(target.board_id)
        .map_err(|error| AppError::from_db(ErrorKind::Save, FAILED, &error))?;
    other
        .add_card(target.column_id, title, "")
        .map_err(|error| AppError::from_board(FAILED, &error))?;
    store
        .save_board(&mut other)
        .map_err(|error| AppError::from_save(&error))?;
    // 置き場所を放してから読み直す（`AppState::store` の注意書き）。
    drop(store);
    state.snapshot()
}

pub fn set_capture_target(
    state: &AppState,
    target: Option<(BoardId, ColumnId)>,
) -> Result<(), AppError> {
    store(
        state,
        "カードの追加先を覚えられませんでした",
        |store| store.set_capture_target(target),
    )
}

/// 開いているボードのカラムを、キャプチャ先にする。`None` で既定に戻す。
///
/// 画面はカラムしか知らないので、ボードを足すのはここです。返すスナップショットで
/// 「⚡ クイックキャプチャ先」の印が動きます。
pub fn set_capture_column(
    state: &AppState,
    column_id: Option<ColumnId>,
) -> Result<Snapshot, AppError> {
    let target = column_id.map(|column_id| (state.lock().id, column_id));
    set_capture_target(state, target)?;
    state.snapshot()
}

/// 保存されている割り当てを読む。無ければ `None`。
pub fn quick_capture_shortcut(state: &AppState) -> Result<Option<String>, AppError> {
    let store = state.store().map_err(|error| {
        AppError::from_db(ErrorKind::Save, "割り当てを読めませんでした", &error)
    })?;
    store
        .load_quick_capture_shortcut()
        .map_err(|error| AppError::from_db(ErrorKind::Save, "割り当てを読めませんでした", &error))
}

/// 割り当てを覚える。**登録できなかった割り当ては保存しません**（`docs/DESIGN.md`「クイックキャプチャ」）ので、
/// 呼ぶ側が登録に成功してからここを呼びます。読めない文字列はここで断ります。
pub fn set_quick_capture_shortcut(
    state: &AppState,
    shortcut: Option<&str>,
) -> Result<(), AppError> {
    store(
        state,
        "割り当てを覚えられませんでした",
        |store| store.set_quick_capture_shortcut(shortcut),
    )
}

// ---------------------------------------------------------------- 記録

/// webview の未捕捉例外を、Rust 側と同じログに落とす（`docs/DESIGN.md`「アプリが伝えること」）。
///
/// webview の失敗が黙って消えると、原因を追う手段がなくなります。
pub fn log_frontend_error(message: &str) {
    diagnostics::log(&format!("webview: {message}"));
}

/// その日ぶんの控えを 1 つ残す。起動のときに別スレッドから呼びます。
///
/// 失敗しても起動は止めません（`docs/DESIGN.md`）。取るのは起動時で、終了時では
/// ない——終了時に取ると、壊した状態のほうを保存することになります。
#[cfg(feature = "shell")]
pub fn run_daily_backup(database_path: &Path) {
    if let Err(error) = backup::run_daily(database_path, Local::now().date_naive()) {
        diagnostics::log(&format!(
            "failed to back up {}: {error}",
            database_path.display()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 拡張子を落として保存されたファイルは、次に開くときに何か分からない。
    #[test]
    fn adds_the_extension_when_the_chosen_name_has_none() {
        assert_eq!(
            with_extension(Path::new("/tmp/board"), "json"),
            PathBuf::from("/tmp/board.json")
        );
    }

    /// すでに付いているものは触らない。`board.json.txt` を選んだ意図を書き換えない。
    #[test]
    fn keeps_the_extension_the_person_chose() {
        assert_eq!(
            with_extension(Path::new("/tmp/board.txt"), "json"),
            PathBuf::from("/tmp/board.txt")
        );
    }

    /// 説明はユーザーが打った文字列なので、開く前に確かめる（ADR 0002）。
    #[test]
    fn opens_only_http_and_https() {
        assert_eq!(
            openable_url("https://example.com"),
            Some("https://example.com")
        );
        assert_eq!(
            openable_url("http://example.com"),
            Some("http://example.com")
        );
        assert_eq!(openable_url("file:///etc/passwd"), None);
        assert_eq!(openable_url("javascript:alert(1)"), None);
        assert_eq!(openable_url("example.com"), None);
    }
}
