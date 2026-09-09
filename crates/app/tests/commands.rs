//! `docs/DESIGN.md`「コマンドとイベント」のコマンドを、外から呼んで確かめる。
//!
//! **盤面の操作はここにありません**（[ADR 0039]）。カードを足す・動かす・戻す
//! は webview のモデル（`web/src/model/board.ts`）にあり、Vitest が見ています。
//! こちらが見るのは、ウェブアプリならサーバ側に書くだろうもの——置き場所の
//! 読み書きと、覚えておく設定、書き出しと控えです。
//!
//! SQLite は毎回**開き直して**読みます。メモリ上の値を覗くと、保存を忘れた
//! コマンドがテストの中でだけ通ってしまいます。
//!
//! [ADR 0039]: ../../docs/adr/0039-the-board-model-moves-to-typescript.md

use std::path::PathBuf;

use ekanban_app::commands;
use ekanban_app::commands::BoardDocument;
use ekanban_app::error::{ErrorKind, Field};
use ekanban_app::snapshot::{CaptureTarget, ThemePreference};
use ekanban_app::state::Source;
use ekanban_app::AppState;
use ekanban_core::db::{Database, FilterState, WindowBoundsState};
use ekanban_core::model::{
    Board, BoardId, CardEvent, CardEventKind, CardId, PreviousPolicy, Recurrence, Schedule,
};
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
        let (state, _) = commands::load_startup_state(Source::Sqlite(path.clone()))
            .expect("the startup state is read");
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

    /// webview が受け取るのと同じ形の、先頭のボード。
    fn document(&self) -> BoardDocument {
        commands::load_documents(&self.state)
            .expect("the documents load")
            .remove(0)
    }

    fn first_card(&self) -> CardId {
        self.stored().columns[0].cards[0].id
    }
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

    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");

    assert_eq!(startup.theme, ThemePreference::Dark);
    assert!(startup.sidebar_collapsed);
    assert_eq!(startup.window_bounds.map(|b| b.width), Some(900.));
    // **盤面は入っていません。** 開くボードだけを渡し、中身は `load_documents`
    // で読みます（ADR 0039）。
    assert_eq!(startup.open_board_id, harness.stored().id);
    assert_eq!(startup.capture_target, None);
    assert_eq!(startup.quick_capture_shortcut, None);
    // 何も選ばれていなければ、日は午前 4 時に変わる（ADR 0048）。
    assert_eq!(startup.day_boundary_hour, 4);
}

/// 日付の切り替わりを選ぶと、次の起動でそれが返る（ADR 0048）。
#[test]
fn the_startup_state_carries_the_chosen_day_boundary_hour() {
    let harness = Harness::open();

    commands::set_day_boundary_hour(&harness.state, 0).expect("stored");

    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");
    assert_eq!(startup.day_boundary_hour, 0, "0 時境界に戻せる");

    // 24 時は無い。断られた値は覚えない。
    assert!(commands::set_day_boundary_hour(&harness.state, 24).is_err());
    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");
    assert_eq!(startup.day_boundary_hour, 0);
}

/// 最後に開いていたボードが消えていたら、先頭のボードに黙って戻る。
#[test]
fn a_missing_last_board_falls_back_to_the_first_one() {
    let harness = Harness::open();
    let created = commands::create_board(&harness.state, "2 つ目").expect("a board is created");
    let created_id = created.board.id;
    commands::delete_board(&harness.state, created_id).expect("the board is deleted");

    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");
    assert_ne!(startup.open_board_id, created_id);
}

// ---------------------------------------------------------------- ボード

/// ボードを作る・開いていたものを覚える・消す。
///
/// **名前を変えるのはここにありません**——それは盤面の操作なので、webview が
/// 当てて `save_document` で書きます（ADR 0039）。
#[test]
fn creating_remembering_and_deleting_boards() {
    let harness = Harness::open();
    let first_id = harness.stored().id;

    let created = commands::create_board(&harness.state, "2 つ目").expect("a board is created");
    assert_eq!(created.board.name, "2 つ目");
    // 作ったばかりの文書は、そのまま手元の一覧へ足せる形で返る。
    assert_eq!(created.next_card_id, created.board.next_card_id);
    assert_eq!(commands::load_documents(&harness.state).unwrap().len(), 2);

    commands::set_open_board(&harness.state, first_id).expect("the open board is remembered");
    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");
    assert_eq!(startup.open_board_id, first_id);

    commands::delete_board(&harness.state, created.board.id).expect("the board is deleted");
    assert_eq!(commands::load_documents(&harness.state).unwrap().len(), 1);
}

/// 最後の 1 つは消せない。開く相手がいなくなる。
#[test]
fn the_last_board_cannot_be_deleted() {
    let harness = Harness::open();
    let only = harness.stored().id;
    commands::delete_board(&harness.state, only).expect_err("the last board is refused");
    assert_eq!(commands::load_documents(&harness.state).unwrap().len(), 1);
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

    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");
    assert_eq!(startup.filter.search, "SQLite");
    assert_eq!(startup.theme, ThemePreference::Dark);
    assert!(startup.sidebar_collapsed);
    assert_eq!(startup.window_bounds.map(|b| b.height), Some(500.));
    assert_eq!(startup.day_boundary_hour, 4);
}

// ---------------------------------------------------------------- 盤面の読み書き

/// 読んで、書いて、読み直すと同じもの（[ADR 0039]）。**採番の続きも運びます**
/// ——webview が手元で番号を採るのに要るので、`Board` の `#[serde(skip)]` の
/// 外側に出してあります。
///
/// [ADR 0039]: ../../docs/adr/0039-the-board-model-moves-to-typescript.md
#[test]
fn a_document_round_trips_through_the_store() {
    let harness = Harness::open();

    let document = harness.document();
    assert_eq!(document.board.id, harness.stored().id);
    assert_eq!(document.next_card_id, harness.stored().next_card_id);

    let saved = commands::save_document(&harness.state, document.clone(), Vec::new())
        .expect("the document is saved");
    assert_eq!(saved.rev, document.rev + 1, "書くたびに版が 1 つ進む");

    let after = harness.document();
    assert_eq!(after.board, document.board);
    assert_eq!(after.next_card_id, document.next_card_id);
    assert_eq!(after.rev, saved.rev);
}

/// webview が当てた盤面が、そのまま置き場所に届く。
///
/// **中身は見直しません**（ADR 0039）。行として成り立っているかだけを見ます。
#[test]
fn what_the_webview_changed_reaches_sqlite() {
    let harness = Harness::open();
    let mut document = harness.document();
    document.board.name = "画面が名前を変えた".to_string();
    let card = document.board.columns[0].cards[0].id;
    document.board.columns[0].cards[0].title = "画面が書き換えた".to_string();

    commands::save_document(&harness.state, document, Vec::new()).expect("the document is saved");

    let stored = harness.stored();
    assert_eq!(stored.name, "画面が名前を変えた");
    assert_eq!(stored.columns[0].cards[0].id, card);
    assert_eq!(stored.columns[0].cards[0].title, "画面が書き換えた");
}

/// 積まれた履歴は、そのまま追記される。**中身は見直しません**（ADR 0039）。
#[test]
fn the_events_the_webview_stacked_are_appended_as_they_are() {
    let harness = Harness::open();
    let document = harness.document();
    let board_id = document.board.id;
    let card_id = harness.first_card();

    commands::save_document(
        &harness.state,
        document,
        vec![CardEvent {
            card_id,
            kind: CardEventKind::Moved,
            from_column_id: Some(1),
            to_column_id: Some(2),
            at: 1_700_000_000_000,
        }],
    )
    .expect("the document is saved");

    let contents =
        commands::export_board_json_contents(&harness.state, board_id).expect("the JSON is built");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
    // 土台のボードを作ったときの `created` が既に並んでいる。足したものは最後。
    let events = parsed["card_events"]
        .as_array()
        .expect("the events are there");
    let last = events.last().expect("the event that was just saved");
    assert_eq!(last["kind"], "moved");
    assert_eq!(last["card_id"], card_id);
    assert_eq!(last["from_column_id"], 1);
    assert_eq!(last["to_column_id"], 2);
}

/// ほかの窓が先に書いていたら、**書かずに断る**（[ADR 0040]）。
///
/// 断ったあとの置き場所には、先に書いたほうの内容が残っている。
///
/// [ADR 0040]: ../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
#[test]
fn saving_an_old_copy_is_refused_and_changes_nothing() {
    let harness = Harness::open();
    let document = harness.document();

    // 1 つ目の窓が書く。
    let mut first = document.clone();
    first.board.name = "先に書いたほう".to_string();
    commands::save_document(&harness.state, first, Vec::new()).expect("the first save goes in");

    // 2 つ目の窓は、読んだときの版のまま書こうとする。
    let mut second = document.clone();
    second.board.name = "あとから書いたほう".to_string();
    let failure = commands::save_document(&harness.state, second, Vec::new())
        .expect_err("the stale copy is refused");

    assert_eq!(failure.kind, ErrorKind::Save);
    assert_eq!(harness.stored().name, "先に書いたほう");
}

/// 受け取った盤面が行として成り立っていなければ、書かずに断る（ADR 0040）。
///
/// **盤面の判断はやり直しません。** 見るのは「そのまま書けるか」だけ。
#[test]
fn a_board_that_does_not_hold_together_is_refused() {
    let harness = Harness::open();
    let before = harness.stored();
    let document = harness.document();

    let mut empty_title = document.clone();
    empty_title.board.columns[0].cards[0].title = "  ".to_string();
    let failure = commands::save_document(&harness.state, empty_title, Vec::new())
        .expect_err("an empty title is refused");
    assert_eq!(failure.field, Some(Field::CardTitle));

    let mut wrong_position = document.clone();
    wrong_position.board.columns[0].cards[0].position = 7;
    let failure = commands::save_document(&harness.state, wrong_position, Vec::new())
        .expect_err("a position that disagrees with the order is refused");
    assert_eq!(failure.kind, ErrorKind::BoardIo);

    let mut unknown_tag = document.clone();
    unknown_tag.board.columns[0].cards[0].tag_ids = vec![999];
    commands::save_document(&harness.state, unknown_tag, Vec::new())
        .expect_err("a card pointing at a tag that is not there is refused");

    let mut behind_counter = document.clone();
    behind_counter.next_card_id = 1;
    commands::save_document(&harness.state, behind_counter, Vec::new())
        .expect_err("a counter that would hand out an id already in use is refused");

    // どれも書かれていない。
    assert_eq!(harness.stored(), before);
}

/// 繰り返しの定義が、`save_document` の 1 本の口から SQLite まで届く（#198）。
///
/// **判断は webview 側**です（`web/src/model/recurrence.ts`）。ここが見るのは、
/// 画面が組み立てた定義がそのまま行として着地することだけ。
#[test]
fn a_recurrence_the_webview_made_reaches_sqlite() {
    let harness = Harness::open();
    let mut document = harness.document();
    let column_id = document.board.columns[0].id;
    let now = 1_700_000_000_000;

    document.board.recurrences.push(Recurrence {
        id: document.next_recurrence_id,
        board_id: document.board.id,
        title: "週次の振り返り".to_string(),
        description: "今週やったことを 10 行で。".to_string(),
        column_id,
        tag_ids: Vec::new(),
        checklist: vec!["やったことを並べる".to_string()],
        schedule: Schedule::Weekly { days: vec![0, 4] },
        lead_days: 2,
        previous: PreviousPolicy::Delete,
        enabled: true,
        last_generated_on: chrono::NaiveDate::from_ymd_opt(2026, 2, 13),
        created_at: now,
        updated_at: now,
    });
    document.next_recurrence_id += 1;
    let recurrence_id = document.board.recurrences[0].id;
    // 出来たカードは、定義と発生日を参照として持つ。
    document.board.columns[0].cards[0].recurrence_id = Some(recurrence_id);
    document.board.columns[0].cards[0].occurrence_date =
        chrono::NaiveDate::from_ymd_opt(2026, 2, 13);
    let expected = document.board.recurrences.clone();

    commands::save_document(&harness.state, document, Vec::new()).expect("the board is saved");

    let stored = harness.stored();
    assert_eq!(stored.recurrences, expected, "定義がそのまま届く");
    assert_eq!(
        stored.columns[0].cards[0].recurrence_id,
        Some(recurrence_id)
    );
    assert_eq!(
        stored.columns[0].cards[0].occurrence_date,
        chrono::NaiveDate::from_ymd_opt(2026, 2, 13)
    );
}

/// 繰り返しの定義も、行として成り立っているかを検める（[ADR 0040]）。
///
/// **カードが指す定義が実在するかは見ません**——定義を消しても盤面のカードは
/// 残るので、そこを見ると消した瞬間から保存できなくなります。
///
/// [ADR 0040]: ../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
#[test]
fn a_recurrence_that_does_not_hold_together_is_refused() {
    let harness = Harness::open();
    let before = harness.stored();
    let document = harness.document();
    let column_id = document.board.columns[0].id;
    let now = 1_700_000_000_000;
    let recurrence = |over: fn(&mut Recurrence)| {
        let mut document = document.clone();
        let mut recurrence = Recurrence {
            id: document.next_recurrence_id,
            board_id: document.board.id,
            title: "メールを見る".to_string(),
            description: String::new(),
            column_id,
            tag_ids: Vec::new(),
            checklist: Vec::new(),
            schedule: Schedule::Daily,
            lead_days: 0,
            previous: PreviousPolicy::Archive,
            enabled: true,
            last_generated_on: None,
            created_at: now,
            updated_at: now,
        };
        over(&mut recurrence);
        document.board.recurrences.push(recurrence);
        document.next_recurrence_id += 1;
        document
    };

    let failure = commands::save_document(
        &harness.state,
        recurrence(|recurrence| recurrence.title = "  ".to_string()),
        Vec::new(),
    )
    .expect_err("an empty title is refused");
    assert_eq!(failure.field, Some(Field::RecurrenceTitle));

    commands::save_document(
        &harness.state,
        recurrence(|recurrence| recurrence.tag_ids = vec![999]),
        Vec::new(),
    )
    .expect_err("a template pointing at a tag that is not there is refused");

    commands::save_document(
        &harness.state,
        recurrence(|recurrence| recurrence.schedule = Schedule::Weekly { days: Vec::new() }),
        Vec::new(),
    )
    .expect_err("a weekly schedule that names no weekday is refused");

    commands::save_document(
        &harness.state,
        recurrence(|recurrence| recurrence.lead_days = -1),
        Vec::new(),
    )
    .expect_err("looking backwards is refused");

    let mut behind_counter = recurrence(|_| {});
    behind_counter.next_recurrence_id = 0;
    commands::save_document(&harness.state, behind_counter, Vec::new())
        .expect_err("a counter that would hand out an id already in use is refused");

    // カードが消えた定義を指しているのは、断る理由になりません。
    let mut orphan = document.clone();
    orphan.board.columns[0].cards[0].recurrence_id = Some(999);
    commands::save_document(&harness.state, orphan, Vec::new())
        .expect("a card left over from a deleted recurrence is fine");

    let stored = harness.stored();
    assert_eq!(
        stored.recurrences, before.recurrences,
        "どれも書かれていない"
    );
}

// ---------------------------------------------------------------- ファイル

/// JSON の書き出しは、**置いてある形の写し**（ADR 0045）。カードの履歴まで
/// 入り、それは置き場所にしか無いので、組み立てるのも書くのもこちら側。
#[test]
fn exporting_json_writes_a_file_that_can_be_read_back() {
    let harness = Harness::open();
    let directory = tempfile::tempdir().expect("a temporary directory");

    let json = directory.path().join("board.json");
    let written = commands::export_board_json(&harness.state, harness.stored().id, &json)
        .expect("the JSON is written");
    assert_eq!(written, json);

    let contents = std::fs::read_to_string(&json).expect("the file is readable");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
    assert!(parsed.get("columns").is_some());
    assert!(
        parsed.get("card_events").is_some(),
        "カードの履歴が入る。画面が持っているのはいまの盤面だけ"
    );
}

/// 書き出すのは**頼まれたボード**。開いているボードを Rust は知らない（ADR 0039）。
#[test]
fn exporting_json_writes_the_board_it_was_asked_for() {
    let harness = Harness::open();
    let other = commands::create_board(&harness.state, "2 つ目")
        .expect("a board is created")
        .board;

    let contents =
        commands::export_board_json_contents(&harness.state, other.id).expect("the JSON is built");
    let parsed: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
    assert_eq!(parsed["board"]["name"], "2 つ目");
}

/// 組み立てた中身は、そのまま受け取って書く（ADR 0045）。読み方も形も見ない。
#[test]
fn writing_a_text_file_puts_back_exactly_what_it_was_given() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let destination = directory.path().join("board.md");

    let written = commands::write_text_file(&destination, "md", "# 個人 Kanban\n")
        .expect("the file is written");

    assert_eq!(written, destination);
    assert_eq!(
        std::fs::read_to_string(&destination).expect("the file is readable"),
        "# 個人 Kanban\n"
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
fn an_export_gets_the_extension_it_was_told() {
    let directory = tempfile::tempdir().expect("a temporary directory");

    let written = commands::write_text_file(&directory.path().join("board"), "md", "# 盤面\n")
        .expect("the board is written");

    assert_eq!(written.extension().and_then(|it| it.to_str()), Some("md"));
    assert!(written.is_file(), "the file is written where it says");
}

// ---------------------------------------------------------------- キャプチャ

/// キャプチャ先は、覚えて、そのまま返すだけ（ADR 0039）。
///
/// **既定に落とすのも、消えていたときに戻すのも webview** です。盤面を全部
/// 持っているのはそちらなので、往復せずに決められます。
#[test]
fn the_capture_target_is_remembered_and_handed_back_as_it_is() {
    let harness = Harness::open();
    let stored = harness.stored();
    let column_id = stored.columns[1].id;

    assert_eq!(
        commands::capture_target(&harness.state).expect("the target is read"),
        None,
        "選ばれていなければ何も言わない"
    );

    commands::set_capture_target(&harness.state, Some((stored.id, column_id)))
        .expect("the target is stored");
    assert_eq!(
        commands::capture_target(&harness.state).expect("the target is read"),
        Some(CaptureTarget {
            board_id: stored.id,
            column_id,
        })
    );

    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");
    assert_eq!(
        startup.capture_target.map(|target| target.column_id),
        Some(column_id)
    );

    commands::set_capture_target(&harness.state, None).expect("the target is cleared");
    assert_eq!(
        commands::capture_target(&harness.state).expect("the target is read"),
        None
    );
}

/// 消えたカラムを指したままでも、置き場所は黙って返す。**直すのは画面**。
#[test]
fn a_capture_target_that_no_longer_exists_comes_back_unchanged() {
    let harness = Harness::open();
    let stored = harness.stored();
    let column_id = stored.columns[2].id;
    commands::set_capture_target(&harness.state, Some((stored.id, column_id)))
        .expect("the target is stored");

    let mut document = harness.document();
    document
        .board
        .columns
        .retain(|column| column.id != column_id);
    for (at, column) in document.board.columns.iter_mut().enumerate() {
        column.position = i64::try_from(at).expect("a column index fits");
    }
    commands::save_document(&harness.state, document, Vec::new()).expect("the column is removed");

    assert_eq!(
        commands::capture_target(&harness.state)
            .expect("the target is read")
            .map(|target| target.column_id),
        Some(column_id),
        "置き場所は覚えたものをそのまま返す"
    );
}

#[test]
fn the_quick_capture_shortcut_is_remembered_as_it_was_given() {
    let harness = Harness::open();
    // 押された組み合わせが、そのままの形で `app_state` に入る。**既にある
    // データベースの割り当てを読み続けられることが、この形を変えない理由**
    // （`shortcut.rs`）。
    commands::set_quick_capture_shortcut(&harness.state, Some("ctrl-shift-n")).expect("stored");

    let (_, startup) = commands::load_startup_state(Source::Sqlite(harness.path.clone()))
        .expect("the state is read");
    assert_eq!(
        startup.quick_capture_shortcut.as_deref(),
        Some("ctrl-shift-n")
    );
}

/// ボードを跨いだ書き込みも、同じ 1 本の口を通る（ADR 0039）。
///
/// クイックキャプチャの窓は、開いているボード以外にも書きます。Rust から見れば
/// 「頼まれた文書を書く」だけで、どの窓が呼んだかは関係ありません。
#[test]
fn a_board_that_is_not_open_is_written_through_the_same_door() {
    let harness = Harness::open();
    let other_id: BoardId = commands::create_board(&harness.state, "受け皿")
        .expect("a board is created")
        .board
        .id;

    let mut other = commands::load_documents(&harness.state)
        .expect("the documents load")
        .into_iter()
        .find(|document| document.board.id == other_id)
        .expect("the new board is there");
    other.board.name = "別の窓が書いた".to_string();
    commands::save_document(&harness.state, other, Vec::new()).expect("the document is saved");

    let written = Database::open(&harness.path)
        .expect("the database opens")
        .load_board_by_id(other_id)
        .expect("the other board loads");
    assert_eq!(written.name, "別の窓が書いた");
}
