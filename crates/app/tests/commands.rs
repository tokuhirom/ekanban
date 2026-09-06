//! `docs/DESIGN.md`「コマンドとイベント」のコマンドを、外から呼んで確かめる。
//!
//! 見るのは 2 つです。**返ってきたスナップショット**と、**SQLite に何が入ったか**。
//! 保存はコマンドの中で終わるので（`docs/DESIGN.md`「状態の持ち主」）、待ち合わせも巻き戻しも要りません。
//! 開き直したデータベースから読むのは、gpui 版の `Harness::stored_board` と
//! 同じやり方です。

use std::path::PathBuf;

use ekanban_app::commands;
use ekanban_app::error::{ErrorKind, Field};
use ekanban_app::snapshot::ThemePreference;
use ekanban_app::AppState;
use ekanban_core::db::{Database, FilterState, WindowBoundsState};
use ekanban_core::model::{Board, CardId, ChecklistItemDraft, ColumnId};
use tempfile::TempDir;

struct Harness {
    _directory: TempDir,
    path: PathBuf,
    state: AppState,
}

impl Harness {
    /// カードの入ったボードを持つデータベースを開く。
    fn open() -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory is available");
        let path = directory.path().join("board.sqlite3");
        {
            let mut database = Database::open(&path).expect("a new database is created");
            let mut fixture = Board::fixture();
            database
                .save_board(&mut fixture)
                .expect("the fixture board is saved");
        }
        let (state, _) = commands::load_startup_state(&path).expect("the startup state is read");
        Self {
            _directory: directory,
            path,
            state,
        }
    }

    /// SQLite に入っている盤面。開き直して読む。
    fn stored(&self) -> Board {
        Database::open(&self.path)
            .expect("the database opens")
            .load_board()
            .expect("the board loads")
    }

    fn first_column(&self) -> ColumnId {
        self.stored().columns[0].id
    }

    fn first_card(&self) -> CardId {
        self.stored().columns[0].cards[0].id
    }
}

fn titles(board: &Board, column: usize) -> Vec<String> {
    board.columns[column]
        .cards
        .iter()
        .map(|card| card.title.clone())
        .collect()
}

// ---------------------------------------------------------------- 起動

#[test]
fn the_startup_state_carries_everything_the_window_needs_to_open() {
    let harness = Harness::open();
    {
        let database = Database::open(&harness.path).expect("the database opens");
        database.set_theme_preference("dark").expect("stored");
        database.set_sidebar_collapsed(true).expect("stored");
        database
            .set_window_bounds(WindowBoundsState {
                x: 10.,
                y: 20.,
                width: 900.,
                height: 600.,
            })
            .expect("stored");
    }

    let (_, startup) = commands::load_startup_state(&harness.path).expect("the state is read");

    assert_eq!(startup.theme, ThemePreference::Dark);
    assert!(startup.sidebar_collapsed);
    assert_eq!(startup.window_bounds.map(|b| b.width), Some(900.));
    assert_eq!(startup.snapshot.board, harness.stored());
    assert!(!startup.snapshot.boards.is_empty());
    assert!(!startup.snapshot.can_undo, "開いた直後に戻せるものはない");
    assert_eq!(startup.capture_target, None);
    assert_eq!(startup.quick_capture_shortcut, None);
}

/// 最後に開いていたボードが消えていたら、先頭のボードに黙って戻る。
#[test]
fn a_missing_last_board_falls_back_to_the_first_one() {
    let harness = Harness::open();
    let created = commands::create_board(&harness.state, "2 つ目").expect("a board is created");
    let created_id = created.board.id;
    commands::delete_board(&harness.state, created_id).expect("the board is deleted");

    let (_, startup) = commands::load_startup_state(&harness.path).expect("the state is read");
    assert_ne!(startup.snapshot.board.id, created_id);
}

// ---------------------------------------------------------------- ボード

#[test]
fn creating_switching_renaming_and_deleting_boards() {
    let harness = Harness::open();
    let first_id = harness.state.snapshot().expect("a snapshot").board.id;

    let created = commands::create_board(&harness.state, "2 つ目").expect("a board is created");
    assert_eq!(created.board.name, "2 つ目");
    assert_eq!(created.boards.len(), 2);

    let renamed = commands::rename_board(&harness.state, "名前を変えた").expect("renamed");
    assert_eq!(renamed.board.name, "名前を変えた");
    assert_eq!(harness.stored().name, "名前を変えた", "保存まで届いている");

    let switched = commands::switch_board(&harness.state, first_id).expect("switched");
    assert_eq!(switched.board.id, first_id);

    let deleted = commands::delete_board(&harness.state, created.board.id).expect("deleted");
    assert_eq!(deleted.boards.len(), 1);
    assert_eq!(deleted.board.id, first_id);
}

#[test]
fn an_empty_board_name_lands_next_to_the_field_that_took_it() {
    let harness = Harness::open();
    let error =
        commands::rename_board(&harness.state, "   ").expect_err("an empty name is refused");
    assert_eq!(error.kind, ErrorKind::Validation);
    assert_eq!(error.field, Some(Field::BoardName));
}

// ---------------------------------------------------------------- カード

#[test]
fn adding_editing_moving_copying_and_deleting_a_card() {
    let harness = Harness::open();
    let column = harness.first_column();

    let added = commands::add_card(&harness.state, column, "足したカード", "説明")
        .expect("the card is added");
    assert!(titles(&added.board, 0).contains(&"足したカード".to_string()));
    assert!(added.can_undo, "足したら戻せる");
    assert!(titles(&harness.stored(), 0).contains(&"足したカード".to_string()));

    let card_id = added.board.columns[0]
        .cards
        .iter()
        .find(|card| card.title == "足したカード")
        .expect("the card is on the board")
        .id;

    let updated = commands::update_card(
        &harness.state,
        card_id,
        "書き換えた",
        "新しい説明",
        "2026-03-04",
        Vec::new(),
        vec![ChecklistItemDraft {
            id: None,
            text: "項目".to_string(),
            checked: false,
        }],
    )
    .expect("the card is updated");
    let card = updated.board.columns[0]
        .cards
        .iter()
        .find(|card| card.id == card_id)
        .expect("the card is still there");
    assert_eq!(card.title, "書き換えた");
    assert_eq!(
        card.due_date.map(|d| d.to_string()).as_deref(),
        Some("2026-03-04")
    );
    assert_eq!(card.checklist_items.len(), 1);

    let second_column = harness.stored().columns[1].id;
    let moved =
        commands::move_card(&harness.state, card_id, second_column, 0).expect("the card is moved");
    assert_eq!(moved.board.columns[1].cards[0].id, card_id);
    assert_eq!(harness.stored().columns[1].cards[0].id, card_id);

    let before_copy = moved.board.columns[1].cards.len();
    let copied = commands::copy_card(&harness.state, card_id).expect("the card is copied");
    assert_eq!(copied.board.columns[1].cards.len(), before_copy + 1);

    let deleted = commands::delete_card(&harness.state, card_id).expect("the card is deleted");
    assert!(!deleted.board.columns[1]
        .cards
        .iter()
        .any(|card| card.id == card_id));
}

#[test]
fn archiving_and_restoring_a_card() {
    let harness = Harness::open();
    let card_id = harness.first_card();

    let archived = commands::archive_card(&harness.state, card_id).expect("archived");
    assert!(archived
        .board
        .archived_cards
        .iter()
        .any(|c| c.id == card_id));
    assert!(harness
        .stored()
        .archived_cards
        .iter()
        .any(|c| c.id == card_id));

    let restored = commands::restore_card(&harness.state, card_id).expect("restored");
    assert!(!restored
        .board
        .archived_cards
        .iter()
        .any(|c| c.id == card_id));
}

#[test]
fn setting_a_due_date_and_tags_on_a_card() {
    let harness = Harness::open();
    let card_id = harness.first_card();

    commands::set_card_due_date(&harness.state, card_id, "2026-01-02").expect("the date is set");
    let stored = harness.stored();
    let card = stored.columns[0]
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .unwrap();
    assert_eq!(
        card.due_date.map(|d| d.to_string()).as_deref(),
        Some("2026-01-02")
    );

    let tagged = commands::add_tag(&harness.state, "重要", "#60a5fa").expect("a tag is added");
    let tag_id = tagged.board.tags[0].id;
    let snapshot =
        commands::set_card_tags(&harness.state, card_id, vec![tag_id]).expect("tags are set");
    let card = snapshot.board.columns[0]
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .unwrap();
    assert_eq!(card.tag_ids, vec![tag_id]);

    // 空欄は「期限なし」。
    commands::set_card_due_date(&harness.state, card_id, "").expect("the date is cleared");
    let stored = harness.stored();
    assert!(stored.columns[0]
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .unwrap()
        .due_date
        .is_none());
}

/// 読めない期限は、打った入力欄に返す。ダイアログには出さない（`docs/DESIGN.md`「コマンドとイベント」）。
#[test]
fn an_unreadable_due_date_comes_back_to_the_field_with_the_value_that_was_typed() {
    let harness = Harness::open();
    let card_id = harness.first_card();

    let error = commands::set_card_due_date(&harness.state, card_id, "2026/03/04")
        .expect_err("the format is refused");

    assert_eq!(error.kind, ErrorKind::Validation);
    assert_eq!(error.field, Some(Field::DueDate));
    assert_eq!(error.value.as_deref(), Some("2026/03/04"));
}

/// 失敗したら盤面は動かない。画面には何も届いていないので、戻すものもない（`docs/DESIGN.md`「状態の持ち主」）。
#[test]
fn a_refused_operation_leaves_the_board_exactly_as_it_was() {
    let harness = Harness::open();
    let before = harness.state.snapshot().expect("a snapshot").board;

    let error = commands::add_card(&harness.state, harness.first_column(), "   ", "")
        .expect_err("an empty title is refused");
    assert_eq!(error.field, Some(Field::CardTitle));

    assert_eq!(harness.state.snapshot().expect("a snapshot").board, before);
    assert_eq!(harness.stored(), before);
}

/// 無題のカードは作らない（`docs/DESIGN.md`）。
///
/// gpui 版は先にカードを足して取り下げる経路でこれを守っていた。下書きが
/// webview のものになった以上、断る場所はコマンドの入口しかない（`docs/DESIGN.md`「状態の持ち主」）。
#[test]
fn an_untitled_card_never_reaches_the_board_or_the_database() {
    let harness = Harness::open();
    let column = harness.first_column();
    let before = harness.stored();

    for title in ["", "   ", "\u{3000}"] {
        commands::add_card(&harness.state, column, title, "説明だけある")
            .expect_err("an untitled card is refused");
    }
    commands::set_capture_target(&harness.state, Some((before.id, column)))
        .expect("the target is stored");
    commands::capture_card(&harness.state, "  ").expect_err("an untitled capture is refused");

    assert_eq!(harness.stored(), before);
}

// ---------------------------------------------------------------- カラム

#[test]
fn adding_renaming_moving_sorting_and_removing_columns() {
    let harness = Harness::open();

    let added = commands::add_column(&harness.state, "レビュー").expect("a column is added");
    let column_id = added.board.columns.last().expect("a column").id;
    assert_eq!(added.board.columns.last().unwrap().name, "レビュー");

    commands::rename_column(&harness.state, column_id, "確認").expect("renamed");
    assert_eq!(harness.stored().columns.last().unwrap().name, "確認");

    let moved = commands::move_column(&harness.state, column_id, 0).expect("moved");
    assert_eq!(moved.board.columns[0].id, column_id);

    commands::set_column_wip_limit(&harness.state, column_id, "3").expect("a limit is set");
    assert_eq!(harness.stored().columns[0].wip_limit, Some(3));
    commands::set_column_wip_limit(&harness.state, column_id, "").expect("the limit is cleared");
    assert_eq!(harness.stored().columns[0].wip_limit, None);

    commands::sort_column_by_due_date(&harness.state, harness.first_column())
        .expect("the column is sorted");

    let removed = commands::remove_column(&harness.state, column_id).expect("removed");
    assert!(!removed.board.columns.iter().any(|c| c.id == column_id));
}

#[test]
fn archiving_a_column_moves_its_cards_to_the_archive() {
    let harness = Harness::open();
    let column_id = harness.first_column();
    let count = harness.stored().columns[0].cards.len();
    assert!(count > 0, "テスト用の盤面にはカードが載っている");

    let snapshot = commands::archive_column(&harness.state, column_id).expect("archived");

    assert!(snapshot.board.columns[0].cards.is_empty());
    assert_eq!(snapshot.board.archived_cards.len(), count);
    assert_eq!(harness.stored().archived_cards.len(), count);
}

#[test]
fn an_unreadable_wip_limit_comes_back_to_the_field() {
    let harness = Harness::open();
    let error = commands::set_column_wip_limit(&harness.state, harness.first_column(), "たくさん")
        .expect_err("the value is refused");
    assert_eq!(error.field, Some(Field::WipLimit));
    assert_eq!(error.value.as_deref(), Some("たくさん"));
}

// ---------------------------------------------------------------- タグ

#[test]
fn adding_renaming_recoloring_and_removing_tags() {
    let harness = Harness::open();

    let added = commands::add_tag(&harness.state, "重要", "#60a5fa").expect("a tag is added");
    let tag_id = added.board.tags[0].id;

    commands::rename_tag(&harness.state, tag_id, "急ぎ").expect("renamed");
    assert_eq!(harness.stored().tags[0].name, "急ぎ");

    commands::set_tag_color(&harness.state, tag_id, "#f87171").expect("recolored");
    assert_eq!(harness.stored().tags[0].color, "#f87171");

    let removed = commands::remove_tag(&harness.state, tag_id).expect("removed");
    assert!(removed.board.tags.is_empty());
}

#[test]
fn a_duplicate_tag_name_comes_back_to_the_field() {
    let harness = Harness::open();
    commands::add_tag(&harness.state, "重要", "#60a5fa").expect("a tag is added");
    let error =
        commands::add_tag(&harness.state, "重要", "#f87171").expect_err("the name is taken");
    assert_eq!(error.field, Some(Field::TagName));
}

// ---------------------------------------------------------------- 取り消し

#[test]
fn undo_and_redo_reach_sqlite_as_well_as_the_snapshot() {
    let harness = Harness::open();
    let column = harness.first_column();
    let before = titles(&harness.stored(), 0);

    commands::add_card(&harness.state, column, "戻す対象", "").expect("added");

    let undone = commands::undo(&harness.state).expect("undone");
    assert_eq!(titles(&undone.board, 0), before);
    assert_eq!(titles(&harness.stored(), 0), before, "保存まで戻っている");
    assert!(undone.can_redo);

    let redone = commands::redo(&harness.state).expect("redone");
    assert!(titles(&redone.board, 0).contains(&"戻す対象".to_string()));
    assert!(titles(&harness.stored(), 0).contains(&"戻す対象".to_string()));
}

// ---------------------------------------------------------------- 絞り込み

#[test]
fn filtering_matches_cards_by_text_number_and_tag() {
    let harness = Harness::open();
    let stored = harness.stored();
    let all: Vec<CardId> = stored
        .columns
        .iter()
        .flat_map(|column| column.cards.iter())
        .map(|card| card.id)
        .collect();

    assert_eq!(
        commands::filter_cards(&harness.state, "", None),
        all,
        "空の検索語はすべてに一致する"
    );

    let card_id = all[0];
    let title = stored.columns[0].cards[0].title.clone();
    assert_eq!(
        commands::filter_cards(&harness.state, &title, None),
        vec![card_id]
    );
    assert_eq!(
        commands::filter_cards(&harness.state, &format!("#{card_id}"), None),
        vec![card_id],
        "`#12` はカード番号として読む（ADR 0008）"
    );

    let tagged = commands::add_tag(&harness.state, "重要", "#60a5fa").expect("a tag is added");
    let tag_id = tagged.board.tags[0].id;
    assert!(commands::filter_cards(&harness.state, "", Some(tag_id)).is_empty());
    commands::set_card_tags(&harness.state, card_id, vec![tag_id]).expect("tags are set");
    assert_eq!(
        commands::filter_cards(&harness.state, "", Some(tag_id)),
        vec![card_id]
    );
}

/// 正規化は Rust に残す（`docs/DESIGN.md`「絞り込みと検索」）。全角で打っても半角のカードに当たる。
#[test]
fn filtering_normalizes_full_width_text_the_same_way_the_model_does() {
    let harness = Harness::open();
    let column = harness.first_column();
    let added = commands::add_card(&harness.state, column, "SQLite の設計", "").expect("added");
    let card_id = added.board.columns[0]
        .cards
        .iter()
        .find(|card| card.title == "SQLite の設計")
        .expect("the card is there")
        .id;

    assert!(commands::filter_cards(&harness.state, "ｓｑｌｉｔｅ", None).contains(&card_id));
}

/// アーカイブしたカードも返す。隠すか減光するかは呼ぶ側が決める（ADR 0010）。
#[test]
fn filtering_reaches_archived_cards_too() {
    let harness = Harness::open();
    let card_id = harness.first_card();
    let title = harness.stored().columns[0].cards[0].title.clone();
    commands::archive_card(&harness.state, card_id).expect("archived");

    assert!(commands::filter_cards(&harness.state, &title, None).contains(&card_id));
}

// ---------------------------------------------------------------- 表示の状態

#[test]
fn the_display_state_survives_a_restart() {
    let harness = Harness::open();

    commands::set_filter_state(
        &harness.state,
        &FilterState {
            search: "SQLite".to_string(),
            tag_id: None,
        },
    )
    .expect("the filter is stored");
    commands::set_theme_preference(&harness.state, ThemePreference::Dark).expect("stored");
    commands::set_sidebar_collapsed(&harness.state, true).expect("stored");
    commands::set_window_bounds(
        &harness.state,
        WindowBoundsState {
            x: 1.,
            y: 2.,
            width: 800.,
            height: 500.,
        },
    )
    .expect("stored");

    let (_, startup) = commands::load_startup_state(&harness.path).expect("the state is read");
    assert_eq!(startup.filter.search, "SQLite");
    assert_eq!(startup.theme, ThemePreference::Dark);
    assert!(startup.sidebar_collapsed);
    assert_eq!(startup.window_bounds.map(|b| b.height), Some(500.));
}

// ---------------------------------------------------------------- ファイル

#[test]
fn exporting_writes_a_file_that_can_be_read_back() {
    let harness = Harness::open();
    let directory = tempfile::tempdir().expect("a temporary directory");

    let json = directory.path().join("board.json");
    let written = commands::export_board(&harness.state, commands::ExportFormat::Json, &json)
        .expect("the JSON is written");
    assert_eq!(written, json);
    let contents = std::fs::read_to_string(&json).expect("the file is readable");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
    assert!(parsed.get("columns").is_some());

    let markdown = directory.path().join("board.md");
    commands::export_board(&harness.state, commands::ExportFormat::Markdown, &markdown)
        .expect("the Markdown is written");
    let contents = std::fs::read_to_string(&markdown).expect("the file is readable");
    assert!(contents.starts_with("# "));

    assert_eq!(
        commands::suggested_export_name(&harness.state, commands::ExportFormat::Markdown),
        "個人 Kanban.md"
    );
}

#[test]
fn a_backup_is_a_database_that_opens() {
    let harness = Harness::open();
    let directory = tempfile::tempdir().expect("a temporary directory");
    let destination = directory.path().join("copy.sqlite3");

    commands::backup_database(&harness.state, &destination).expect("the backup is written");

    let copied = Database::open(&destination)
        .expect("the copy opens")
        .load_board()
        .expect("the board loads");
    assert_eq!(copied, harness.stored());
}

#[test]
fn the_places_the_app_can_open_point_at_real_paths() {
    let harness = Harness::open();
    assert_eq!(commands::database_location(&harness.state), harness.path);
    assert_eq!(commands::reveal_database(&harness.state), harness.path);

    // 控えの置き場所は、1 つも取れていないうちは「開く先が無い」。
    let backups = harness
        .path
        .parent()
        .expect("the database has a parent")
        .join("backups");
    assert_eq!(commands::reveal_backups(&harness.state), None);
    std::fs::create_dir_all(&backups).expect("the backup directory is created");
    assert_eq!(commands::reveal_backups(&harness.state), Some(backups));
}

/// 控えの保存先に、いま開いているデータベースそのものは選べない。
///
/// `backup_to` は上書きで開くので、通してしまうと控えのつもりで元のファイルを
/// 触ることになる。
#[test]
fn a_backup_refuses_to_overwrite_the_database_it_copies() {
    let harness = Harness::open();
    let failure = commands::backup_database(&harness.state, &harness.path)
        .expect_err("the database itself is refused");
    assert_eq!(failure.kind, ErrorKind::Export);
}

/// 拡張子を落として保存されたファイルは、次に開くときに何か分からない。
#[test]
fn an_export_gets_the_extension_of_its_format() {
    let harness = Harness::open();
    let directory = tempfile::tempdir().expect("a temporary directory");

    let written = commands::export_board(
        &harness.state,
        commands::ExportFormat::Markdown,
        &directory.path().join("board"),
    )
    .expect("the board is written");

    assert_eq!(written.extension().and_then(|it| it.to_str()), Some("md"));
    assert!(written.is_file(), "the file is written where it says");
}

// ---------------------------------------------------------------- キャプチャ

#[test]
fn quick_capture_adds_to_the_chosen_column_through_the_same_save_path() {
    let harness = Harness::open();
    let stored = harness.stored();
    let target_column = stored.columns[1].id;
    commands::set_capture_target(&harness.state, Some((stored.id, target_column)))
        .expect("the target is stored");

    let snapshot = commands::capture_card(&harness.state, "拾ったこと").expect("the card is added");

    assert_eq!(
        snapshot.board.columns[1].cards.last().unwrap().title,
        "拾ったこと",
        "カラムの末尾に足す"
    );
    assert!(snapshot.can_undo, "Undo の対象になる");
    assert_eq!(
        harness.stored().columns[1].cards.last().unwrap().title,
        "拾ったこと"
    );
}

/// キャプチャ先が別のボードでも書ける。開いている盤面はそのまま。
#[test]
fn quick_capture_writes_to_a_board_that_is_not_open() {
    let harness = Harness::open();
    let first = harness.state.snapshot().expect("a snapshot").board;
    let other = commands::create_board(&harness.state, "受け皿")
        .expect("a board is created")
        .board;
    commands::switch_board(&harness.state, first.id).expect("switched back");
    commands::set_capture_target(&harness.state, Some((other.id, other.columns[0].id)))
        .expect("the target is stored");

    let snapshot =
        commands::capture_card(&harness.state, "別のボードへ").expect("the card is added");

    assert_eq!(snapshot.board.id, first.id, "開いている盤面は変わらない");
    let written = Database::open(&harness.path)
        .expect("the database opens")
        .load_board_by_id(other.id)
        .expect("the other board loads");
    assert_eq!(
        written.columns[0].cards.last().unwrap().title,
        "別のボードへ"
    );
}

#[test]
fn quick_capture_falls_back_to_the_first_column_when_no_target_has_been_chosen() {
    let harness = Harness::open();
    // 決まっていないから足せない、にはしない。キャプチャは 1 行を放り込む
    // ためのもので、そこで設定を求めると用が足りない（gpui 版と同じ既定）。
    commands::capture_card(&harness.state, "行き先を決めていない").expect("the card is added");

    let stored = harness.stored();
    assert_eq!(
        stored.columns[0]
            .cards
            .last()
            .map(|card| card.title.as_str()),
        Some("行き先を決めていない")
    );
}

/// キャプチャ先のカラムが消えていたら、黙って未設定に戻す。起動を妨げない。
#[test]
fn a_capture_target_that_no_longer_exists_falls_back_to_none() {
    let harness = Harness::open();
    let stored = harness.stored();
    let column_id = stored.columns[2].id;
    commands::set_capture_target(&harness.state, Some((stored.id, column_id)))
        .expect("the target is stored");
    commands::remove_column(&harness.state, column_id).expect("the column is removed");

    let (_, startup) = commands::load_startup_state(&harness.path).expect("the state is read");
    assert_eq!(startup.capture_target, None);
}

#[test]
fn the_quick_capture_shortcut_is_remembered_as_it_was_given() {
    let harness = Harness::open();
    // 保存の形は gpui 版のまま。**旧いデータベースの割り当てをそのまま読める
    // ことが、この形を変えない理由**（`shortcut.rs`）。
    commands::set_quick_capture_shortcut(&harness.state, Some("ctrl-shift-n")).expect("stored");

    let (_, startup) = commands::load_startup_state(&harness.path).expect("the state is read");
    assert_eq!(
        startup.quick_capture_shortcut.as_deref(),
        Some("ctrl-shift-n")
    );
}
