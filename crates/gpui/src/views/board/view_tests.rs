//! ボードのウィンドウを実際に開いて確かめるテスト。
//!
//! GPUI にはヘッドレスのテスト用プラットフォーム（`TestPlatform`）があり、
//! `#[gpui_kit::test]` を付けると GPU もウィンドウも無いまま `App` と `Window`
//! が立ち上がる。同じモジュールにある純粋関数のテストと違って、ここでは
//! `BoardView` を本物のウィンドウに載せ、キー入力とアクションを流し込んで、
//! 見えている状態と SQLite に書かれた内容の両方を確かめる。
//!
//! 押さえどころ:
//!
//! - 時計は偽物なので、待ち時間は `run_until_parked()` で決める。`sleep` は使わない
//! - 非同期の保存を挟む操作は、確かめる前に必ず `run_until_parked()` する
//! - 割り当ては `crate::menu::install` が入れた本物を使う。テスト用に定義し直すと、
//!   割り当てを変えたときにテストだけ通ってしまう

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use chrono::{Duration, Local};
use gpui_kit::{
    component::{Root, Theme, WindowExt as _},
    AppContext as _, Entity, TestAppContext, VisualTestContext,
};
use tempfile::TempDir;

use super::{BoardView, ThemePreference};
use crate::{
    actions::{
        AddCard, AddColumn, CancelEdit, ClearSearch, DeleteBoard, FocusSearch, RenameBoard,
        SaveEdit, ToggleArchiveView, ToggleBoardList, Undo, UseDarkTheme,
    },
    db::{Database, FilterState, WindowBoundsState},
    hotkey::QuickCapture,
    model::{Board, CardId},
    views::QuickCaptureState,
};

/// 開いたボードと、その裏にあるデータベース。
///
/// `TempDir` を持ったままにしておかないと、テストの途中で SQLite ファイルごと
/// 消える。
struct Harness {
    _dir: TempDir,
    database_path: PathBuf,
    view: Entity<BoardView>,
}

impl Harness {
    /// 画面ではなくディスクの側を見る。保存まで通ったかを確かめるために使う。
    fn stored_board(&self) -> Board {
        let database = Database::open(&self.database_path).expect("stored database opens");
        database.load_board().expect("the board is stored")
    }

    fn columns_of(&self, cx: &mut VisualTestContext) -> Vec<(String, Vec<String>)> {
        self.view.read_with(cx, |view, _| {
            view.board
                .columns
                .iter()
                .map(|column| {
                    (
                        column.name.clone(),
                        column
                            .cards
                            .iter()
                            .map(|card| card.title.clone())
                            .collect::<Vec<_>>(),
                    )
                })
                .collect()
        })
    }

    /// 今の絞り込みで減光されるカードの題名。ekanban は絞り込みでカードを
    /// 隠さず減光するので、「消えたか」ではなくこちらを見る。
    fn dimmed_titles(&self, cx: &mut VisualTestContext) -> Vec<String> {
        self.view.read_with(cx, |view, _| {
            view.board
                .columns
                .iter()
                .flat_map(|column| column.cards.iter())
                .filter(|card| super::card_is_dimmed(card, &view.search_query, view.tag_filter))
                .map(|card| card.title.clone())
                .collect()
        })
    }

    /// アーカイブ表示に並ぶカードの題名。日ごとの見出しごとに分かれる。
    ///
    /// アーカイブ表示は絞り込みから外れたカードを減光ではなく非表示にするので、
    /// ここは「暗いか」ではなく「並んでいるか」を見る。
    fn archived_titles(&self, cx: &mut VisualTestContext) -> Vec<String> {
        self.view.read_with(cx, |view, _| {
            super::archived_groups(
                &view.board.archived_cards,
                &view.search_query,
                view.tag_filter,
            )
            .into_iter()
            .flat_map(|(_, cards)| cards.into_iter().map(|card| card.title.clone()))
            .collect()
        })
    }

    /// ダイアログが開いているか。
    ///
    /// ヘッダの常時表示をやめたので、アプリが何か言ったかどうかはここで見る
    /// （#86）。伝えるのは失敗と、アプリの外にファイルを書けたときだけなので、
    /// 拒否やキャンセルのあとはここが `false` のままであることが答えになる。
    fn dialog_is_open(&self, cx: &mut VisualTestContext) -> bool {
        cx.update(|window, cx| window.has_active_dialog(cx))
    }

    fn selected_card(&self, cx: &mut VisualTestContext) -> Option<CardId> {
        self.view.read_with(cx, |view, _| view.selected_card)
    }

    fn editing_title(&self, cx: &mut VisualTestContext) -> Option<String> {
        self.view.read_with(cx, |view, cx| {
            view.editing_card
                .as_ref()
                .map(|editor| editor.title.read(cx).value().to_string())
        })
    }
}

/// 空のデータベースにデモボードを作り、それを載せたウィンドウを開く。
///
/// `crate::run` がやっていることのうち、ウィンドウを開くまでを再現する。
/// ルートを `Root` にするのは本番と同じで、確認ダイアログや通知がここに載るため。
fn open_board(cx: &mut TestAppContext) -> (Harness, &mut VisualTestContext) {
    open_seeded_board(cx, |_, _| {})
}

/// デモボードの中身を差し替えてからウィンドウを開く。
///
/// `seed` はウィンドウが開く前に呼ばれる。書き換えた結果を SQLite にも残したい
/// なら、`seed` の中で `save_board` まで済ませておく。
fn open_seeded_board(
    cx: &mut TestAppContext,
    seed: impl FnOnce(&mut Database, &mut Board),
) -> (Harness, &mut VisualTestContext) {
    let dir = tempfile::tempdir().expect("a temporary directory is available");
    let database_path = dir.path().join("board.sqlite3");

    let (board, boards) = {
        let mut database = Database::open(&database_path).expect("a new database is created");
        // 初回のシードはカードの無い 3 カラムだけ（`Board::first_run`）。画面の
        // テストは盤面に中身がある前提なので、窓を開ける前に土台を載せる。
        let mut fixture = Board::fixture();
        database
            .save_board(&mut fixture)
            .expect("the fixture board is stored");
        let mut board = {
            let boards = database.load_boards().expect("the seeded board is listed");
            database
                .load_board_by_id(boards[0].id)
                .expect("the seeded board loads")
        };
        seed(&mut database, &mut board);
        let boards = database.load_boards().expect("the seeded board is listed");
        (board, boards)
    };

    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::sync_system_appearance(None, cx);
        crate::menu::install(cx);
        cx.set_global(QuickCapture::new());
    });

    let built: Rc<RefCell<Option<Entity<BoardView>>>> = Rc::new(RefCell::new(None));
    let sink = built.clone();
    let view_path = database_path.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            BoardView::new(
                board,
                boards,
                view_path,
                FilterState::default(),
                WindowBoundsState {
                    x: 0.,
                    y: 0.,
                    width: 1200.,
                    height: 800.,
                },
                ThemePreference::System,
                false,
                QuickCaptureState {
                    shortcut: None,
                    error: None,
                    capture_target: None,
                },
                window,
                cx,
            )
        });
        *sink.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.run_until_parked();

    let view = built.borrow_mut().take().expect("the board view was built");
    (
        Harness {
            _dir: dir,
            database_path,
            view,
        },
        cx,
    )
}

/// ボードのショートカットは入力欄にフォーカスがあると効かない。キー操作の
/// テストはまずボードへフォーカスを戻す。
fn focus_board(harness: &Harness, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        harness
            .view
            .update(cx, |view, cx| view.focus_handle.focus(window, cx))
    });
    cx.run_until_parked();
}

#[gpui_kit::test]
fn opens_a_window_showing_the_stored_board(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);

    let columns = harness.columns_of(cx);
    let stored = harness.stored_board();
    assert_eq!(
        columns,
        stored
            .columns
            .iter()
            .map(|column| (
                column.name.clone(),
                column
                    .cards
                    .iter()
                    .map(|card| card.title.clone())
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        "the window shows exactly the columns and cards that are stored"
    );
    assert!(
        !harness.dialog_is_open(cx),
        "opening the board says nothing at all"
    );
}

#[gpui_kit::test]
fn renaming_the_board_retitles_the_window(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(RenameBoard);
    cx.run_until_parked();
    // 既定値が入っているので、上書きする前に選択しておく。
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("仕事");
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();

    assert_eq!(
        harness
            .view
            .read_with(cx, |view, _| view.board.name.clone()),
        "仕事"
    );
    assert_eq!(
        cx.window_title().as_deref(),
        Some(super::window_title("仕事").as_str()),
        "the title bar follows the board name"
    );
    assert_eq!(harness.stored_board().name, "仕事");
}

#[gpui_kit::test]
fn arrow_keys_walk_the_selection_through_the_board(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let board = harness.stored_board();
    let first = board.columns[0].cards[0].id;
    let second = board.columns[0].cards[1].id;
    let across = board.columns[1].cards[0].id;

    cx.simulate_keystrokes("down");
    assert_eq!(harness.selected_card(cx), Some(first));

    cx.simulate_keystrokes("down");
    assert_eq!(harness.selected_card(cx), Some(second));

    cx.simulate_keystrokes("up right");
    assert_eq!(harness.selected_card(cx), Some(across));
}

#[gpui_kit::test]
fn enter_opens_the_editor_for_the_selected_card(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let first_title = harness.stored_board().columns[0].cards[0].title.clone();

    cx.simulate_keystrokes("down enter");

    assert_eq!(harness.editing_title(cx).as_deref(), Some(&*first_title));
}

#[gpui_kit::test]
fn adding_a_card_and_saving_it_writes_the_title_to_the_database(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(AddCard);
    cx.run_until_parked();
    assert_eq!(
        harness.editing_title(cx).as_deref(),
        Some(""),
        "a new card starts with an empty title so no placeholder text is stored"
    );

    cx.simulate_input("牛乳を買う");
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();

    let stored = harness.stored_board();
    assert!(
        stored.columns[0]
            .cards
            .iter()
            .any(|card| card.title == "牛乳を買う"),
        "the saved card is in the first column: {:?}",
        stored.columns[0].cards
    );
}

#[gpui_kit::test]
fn pressing_enter_in_the_title_saves_the_new_card(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(AddCard);
    cx.run_until_parked();

    cx.simulate_input("牛乳を買う");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    assert!(
        harness.editing_title(cx).is_none(),
        "the panel closes once the card is saved"
    );
    let stored = harness.stored_board();
    assert!(
        stored.columns[0]
            .cards
            .iter()
            .any(|card| card.title == "牛乳を買う"),
        "Enter in the title field saves without reaching for the save button: {:?}",
        stored.columns[0].cards
    );
}

#[gpui_kit::test]
fn cancelling_a_new_card_leaves_no_trace(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let before = harness.columns_of(cx)[0].1.len();

    cx.dispatch_action(AddCard);
    cx.run_until_parked();
    assert_eq!(harness.columns_of(cx)[0].1.len(), before + 1);

    cx.dispatch_action(CancelEdit);
    cx.run_until_parked();

    assert_eq!(
        harness.columns_of(cx)[0].1.len(),
        before,
        "an untitled card is withdrawn rather than kept"
    );
    assert_eq!(
        harness.stored_board().columns[0].cards.len(),
        before,
        "and it never reached the database"
    );
}

#[gpui_kit::test]
fn an_empty_title_is_refused_instead_of_saved(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(AddCard);
    cx.run_until_parked();
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();

    assert!(
        harness.editing_title(cx).is_some(),
        "the editor stays open so the title can be filled in"
    );
    assert!(
        harness.view.read_with(cx, |view, _| view
            .editing_card
            .as_ref()
            .and_then(|editor| editor.error.clone())
            .is_some()),
        "and the reason is shown on the field"
    );
}

#[gpui_kit::test]
fn undo_takes_back_a_saved_card(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(AddCard);
    cx.run_until_parked();
    cx.simulate_input("あとで消す");
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();
    assert!(harness
        .stored_board()
        .columns
        .iter()
        .any(|column| column.cards.iter().any(|card| card.title == "あとで消す")));

    focus_board(&harness, cx);
    cx.dispatch_action(Undo);
    cx.run_until_parked();

    assert!(
        !harness
            .stored_board()
            .columns
            .iter()
            .any(|column| column.cards.iter().any(|card| card.title == "あとで消す")),
        "undo reaches the database, not only the screen"
    );
}

/// 保存に失敗したら、ダイアログで知らせる（#86）。
///
/// 失敗すると `SaveFailure::Restore*` が編集内容を巻き戻す。以前はそれを
/// ヘッダの小さな文字でしか伝えていなかったので、黙って編集が取り消されたように
/// 見えていた。いちばん見逃されたら困るものが、いちばん見逃されやすかった。
///
/// 失敗させるには、データベースのファイルをディレクトリに差し替える。保存は
/// そのつどパスを開き直す（`save_board_snapshot`）ので、どの OS でも開けなくなる。
#[gpui_kit::test]
fn a_failed_save_opens_a_dialog_instead_of_quietly_undoing_the_edit(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    std::fs::remove_file(&harness.database_path).expect("the database file exists");
    std::fs::create_dir(&harness.database_path).expect("a directory takes its place");

    cx.dispatch_action(AddCard);
    cx.run_until_parked();
    cx.simulate_input("保存できないカード");
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();

    assert!(
        harness.dialog_is_open(cx),
        "a save that failed says so instead of rolling back in silence"
    );
    assert_eq!(
        harness.editing_title(cx).as_deref(),
        Some("保存できないカード"),
        "and what was typed is handed back rather than thrown away"
    );
}

/// 受け入れ条件「ボードを開かなくても、どのボードに期限切れ / 本日期限の
/// カードがあるか分かる」（#62）。
#[gpui_kit::test]
fn the_board_list_shows_the_due_counts_of_a_board_that_is_not_open(cx: &mut TestAppContext) {
    let today = Local::now().date_naive();
    let (harness, cx) = open_seeded_board(cx, |database, _| {
        let mut other = database
            .create_board("仕事")
            .expect("a second board is made");
        let column_id = other.columns[0].id;
        let overdue = other
            .add_card(column_id, "過ぎている", "")
            .expect("the column exists");
        let due_today = other
            .add_card(column_id, "今日まで", "")
            .expect("the column exists");
        other
            .set_card_due_date(overdue, Some(today - Duration::days(1)))
            .expect("the card exists");
        other
            .set_card_due_date(due_today, Some(today))
            .expect("the card exists");
        database
            .save_board(&mut other)
            .expect("the board is stored");
    });

    let counts = harness.view.read_with(cx, |view, _| {
        let other = view
            .boards
            .iter()
            .find(|summary| summary.id != view.board.id)
            .expect("the second board is listed")
            .clone();
        view.due_counts_for(&other, today)
    });

    assert_eq!(counts.overdue, 1);
    assert_eq!(counts.today, 1);

    // 開いているほうは期限を持たないので、行は名前だけになる。
    let open_board = harness.view.read_with(cx, |view, _| {
        let summary = view
            .boards
            .iter()
            .find(|summary| summary.id == view.board.id)
            .expect("the open board is listed")
            .clone();
        view.due_counts_for(&summary, today)
    });
    assert!(open_board.is_empty());
}

#[gpui_kit::test]
fn searching_while_the_archive_is_shown_narrows_the_archive(cx: &mut TestAppContext) {
    let (harness, cx) = open_seeded_board(cx, |database, board| {
        let column_id = board.columns[0].id;
        let kept = board
            .add_card(column_id, "請求書を出す", "")
            .expect("the column exists");
        let dropped = board
            .add_card(column_id, "議事録を書く", "")
            .expect("the column exists");
        board.archive_card(kept).expect("the card can be archived");
        board
            .archive_card(dropped)
            .expect("the card can be archived");
        database.save_board(board).expect("the board is stored");
    });
    focus_board(&harness, cx);

    cx.dispatch_action(ToggleArchiveView);
    cx.run_until_parked();
    assert_eq!(
        harness.archived_titles(cx),
        vec!["請求書を出す".to_string(), "議事録を書く".to_string()],
        "both archived cards are listed before filtering"
    );

    cx.dispatch_action(FocusSearch);
    cx.run_until_parked();
    cx.simulate_input("請求書");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    assert!(
        harness.view.read_with(cx, |view, _| view.show_archived),
        "searching does not leave the archive view"
    );
    assert_eq!(
        harness.archived_titles(cx),
        vec!["請求書を出す".to_string()],
        "the archive lists only what the search matched"
    );
}

#[gpui_kit::test]
fn searching_for_a_card_number_leaves_only_that_card(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let board = harness.stored_board();
    let wanted = &board.columns[0].cards[0];
    let other = &board.columns[0].cards[1];
    assert_ne!(
        wanted.id, other.id,
        "the seeded board has two distinct cards"
    );

    cx.dispatch_action(FocusSearch);
    cx.run_until_parked();
    cx.simulate_input(&format!("#{}", wanted.id));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    let dimmed = harness.dimmed_titles(cx);
    assert!(
        !dimmed.contains(&wanted.title),
        "the card with that number stays lit: {dimmed:?}"
    );
    assert!(
        dimmed.contains(&other.title),
        "every other card is dimmed: {dimmed:?}"
    );
}

#[gpui_kit::test]
fn searching_dims_the_cards_that_do_not_match(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let board = harness.stored_board();
    let wanted = board.columns[0].cards[0].title.clone();
    let other = board.columns[0].cards[1].title.clone();
    assert_ne!(wanted, other, "the demo board has two distinct cards");

    // 検索は Enter で確定する。入力しただけでは絞り込まない。
    cx.dispatch_action(FocusSearch);
    cx.run_until_parked();
    cx.simulate_input(&wanted);
    assert_eq!(
        harness
            .view
            .read_with(cx, |view, _| view.search_query.clone()),
        "",
        "typing alone does not filter yet"
    );

    cx.simulate_keystrokes("enter");
    cx.run_until_parked();

    let dimmed = harness.dimmed_titles(cx);
    assert!(
        !dimmed.contains(&wanted),
        "the matching card stays lit: {dimmed:?}"
    );
    assert!(
        dimmed.contains(&other),
        "the others are dimmed rather than hidden: {dimmed:?}"
    );
    assert_eq!(
        harness.columns_of(cx)[0].1.len(),
        board.columns[0].cards.len(),
        "no card is removed from the column while filtering"
    );

    let database = Database::open(&harness.database_path).expect("the database opens");
    assert_eq!(
        database
            .load_filter_state()
            .expect("the filter is stored")
            .search,
        wanted,
        "the next launch opens with the same filter"
    );

    cx.dispatch_action(ClearSearch);
    cx.run_until_parked();

    assert!(
        harness.dimmed_titles(cx).is_empty(),
        "clearing lights every card again"
    );
    assert_eq!(
        harness
            .view
            .read_with(cx, |view, cx| view.search.read(cx).value().to_string()),
        "",
        "and empties the field, not only the filter"
    );
}

#[gpui_kit::test]
fn the_selected_card_can_be_moved_to_the_next_column(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let board = harness.stored_board();
    let moved = board.columns[0].cards[0].title.clone();

    cx.simulate_keystrokes("down");
    cx.simulate_keystrokes("secondary-alt-right");
    cx.run_until_parked();

    let columns = harness.columns_of(cx);
    assert!(!columns[0].1.contains(&moved), "it left the first column");
    assert!(columns[1].1.contains(&moved), "and arrived in the second");

    let stored = harness.stored_board();
    assert!(!stored.columns[0]
        .cards
        .iter()
        .any(|card| card.title == moved));
    assert!(stored.columns[1]
        .cards
        .iter()
        .any(|card| card.title == moved));
}

#[gpui_kit::test]
fn cards_cannot_be_added_while_the_archive_is_shown(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(ToggleArchiveView);
    cx.run_until_parked();
    let before = harness.columns_of(cx)[0].1.len();

    cx.dispatch_action(AddCard);
    cx.run_until_parked();

    assert_eq!(harness.columns_of(cx)[0].1.len(), before);
    assert!(
        !harness.dialog_is_open(cx),
        "the refusal is silent: the archive view simply has nowhere to add a card"
    );
}

#[gpui_kit::test]
fn hiding_the_board_list_is_remembered_across_a_restart(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(ToggleBoardList);
    cx.run_until_parked();

    assert!(harness.view.read_with(cx, |view, _| view.sidebar_collapsed));
    let database = Database::open(&harness.database_path).expect("the database opens");
    assert!(
        database
            .load_sidebar_collapsed()
            .expect("the flag is stored"),
        "the next launch opens with the list hidden"
    );
}

#[gpui_kit::test]
fn choosing_the_dark_theme_is_remembered_across_a_restart(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(UseDarkTheme);
    cx.run_until_parked();

    assert_eq!(
        harness.view.read_with(cx, |view, _| view.theme_preference),
        ThemePreference::Dark
    );
    let database = Database::open(&harness.database_path).expect("the database opens");
    assert_eq!(
        database
            .load_theme_preference()
            .expect("the preference is stored")
            .as_deref(),
        Some("dark")
    );
}

#[gpui_kit::test]
fn a_new_column_is_added_with_the_name_that_was_typed(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let before = harness.columns_of(cx).len();

    cx.dispatch_action(AddColumn);
    cx.run_until_parked();
    cx.simulate_input("レビュー待ち");
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();

    let columns = harness.columns_of(cx);
    assert_eq!(columns.len(), before + 1);
    assert!(columns.iter().any(|(name, _)| name == "レビュー待ち"));
    assert!(harness
        .stored_board()
        .columns
        .iter()
        .any(|column| column.name == "レビュー待ち"));
}

#[gpui_kit::test]
fn shortcuts_stay_out_of_the_way_while_a_field_has_focus(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.simulate_keystrokes("down");
    let selected = harness.selected_card(cx);
    assert!(selected.is_some());

    cx.dispatch_action(FocusSearch);
    cx.run_until_parked();
    cx.simulate_keystrokes("down");

    assert_eq!(
        harness.selected_card(cx),
        selected,
        "arrow keys belong to the field while it is focused"
    );
}

/// メニューの項目もショートカットの表示も `"Board"` の文脈にぶら下がっている。
/// 開いた時点でどこにもフォーカスが無いと、メニューから飛ばしたアクションが誰にも
/// 届かず、押しても何も起きない（#79）。
#[gpui_kit::test]
fn focuses_the_board_when_the_window_opens(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);

    let focused = cx.update(|window, cx| {
        harness
            .view
            .read_with(cx, |view, _| view.focus_handle.is_focused(window))
    });

    assert!(
        focused,
        "the board holds focus from the start, so menu actions reach it"
    );
}

#[gpui_kit::test]
fn draws_a_menu_bar_where_the_operating_system_does_not(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);

    assert_eq!(
        harness
            .view
            .read_with(cx, |view, _| view.menu_bar.is_some()),
        !cfg!(target_os = "macos"),
        "macOS gets its menu bar from the system; everywhere else the board draws one"
    );
}

/// `AppMenuBar` が読むのは `GlobalState::app_menus` で、`cx.set_menus` はそこまで
/// 運んでくれない。橋渡しが外れるとメニューバーが空のまま出るので、届いていることを
/// 見ておく（#79）。
#[gpui_kit::test]
fn hands_the_menu_definitions_to_the_menu_bar(cx: &mut TestAppContext) {
    let (_harness, cx) = open_board(cx);

    let published = cx.update(|_, cx| {
        gpui_kit::component::GlobalState::global(cx)
            .app_menus()
            .iter()
            .map(|menu| menu.name.to_string())
            .collect::<Vec<_>>()
    });
    let defined = crate::menu::menus()
        .iter()
        .map(|menu| menu.name.to_string())
        .collect::<Vec<_>>();

    assert!(!defined.is_empty(), "the menu bar has menus to show");
    assert_eq!(
        published, defined,
        "the menu bar reads the same definitions the menu bar action does"
    );
}

/// メニューバーは別の部品で、ボードの `…` や右クリックのメニューのことを知らない。
/// 触られたら畳んで、2 枚を同時に出したままにしない（#79）。
#[gpui_kit::test]
fn touching_the_menu_bar_closes_the_column_menu(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let column_id = harness.stored_board().columns[0].id;
    cx.update(|_, cx| {
        harness.view.update(cx, |view, cx| {
            view.toggle_column_context_menu(column_id, cx)
        })
    });
    cx.run_until_parked();
    assert_eq!(
        harness
            .view
            .read_with(cx, |view, _| view.context_menu_column),
        Some(column_id)
    );

    cx.update(|_, cx| {
        harness
            .view
            .update(cx, |view, cx| view.dismiss_board_menus(cx))
    });
    cx.run_until_parked();

    assert_eq!(
        harness
            .view
            .read_with(cx, |view, _| view.context_menu_column),
        None,
        "only one menu is open at a time"
    );
}

#[gpui_kit::test]
fn deleting_the_only_board_is_refused_before_the_dialog(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    let board_id = harness.view.read_with(cx, |view, _| view.board.id);
    assert_eq!(
        harness.view.read_with(cx, |view, _| view.boards.len()),
        1,
        "the seeded database has a single board"
    );

    cx.dispatch_action(DeleteBoard);
    cx.run_until_parked();

    assert!(
        !harness.dialog_is_open(cx),
        "no confirmation dialog is opened for a deletion that would be refused"
    );
    assert_eq!(
        harness.view.read_with(cx, |view, _| view.board.id),
        board_id,
        "and the board is still open"
    );
    assert_eq!(harness.stored_board().id, board_id);
}
/// カードがカラムに収まらない枚数になっても、カラムはウィンドウの高さの中に
/// 収まり、あふれた分はカラムの中で縦スクロールできる。
///
/// 見るのは絶対値ではなく関係。カード一覧の表示領域がウィンドウに収まっている
/// ことと、スクロールできる余地（`max_offset`）が残っていることを確かめるので、
/// カードの高さやテーマが変わっても意味が保たれる。
#[gpui_kit::test]
fn a_column_with_more_cards_than_fit_scrolls_inside_the_window(cx: &mut TestAppContext) {
    let (harness, cx) = open_seeded_board(cx, |database, board| {
        let column_id = board.columns[0].id;
        for index in 0..60 {
            board
                .add_card(column_id, format!("カード {index}"), "")
                .expect("the first column takes a card");
        }
        database
            .save_board(board)
            .expect("the seeded cards are stored");
    });

    let viewport = cx.update(|window, _| window.viewport_size());
    let (list_bounds, max_offset) = harness.view.read_with(cx, |view, _| {
        let column_id = view.board.columns[0].id;
        let handle = view
            .column_scroll_handles
            .get(&column_id)
            .expect("the rendered column has a scroll handle");
        (handle.bounds(), handle.max_offset())
    });

    assert!(
        list_bounds.size.height <= viewport.height,
        "the card list stays inside the window instead of stretching to fit every card: \
         list {list_bounds:?} in a window of {viewport:?}"
    );
    assert!(
        max_offset.y > gpui_kit::px(0.),
        "the cards that do not fit can be reached by scrolling the column: \
         max_offset {max_offset:?}"
    );
}

#[gpui_kit::test]
fn keeps_the_description_editable_with_links_in_it(cx: &mut TestAppContext) {
    let (harness, cx) = open_seeded_board(cx, |database, board| {
        let column_id = board.columns[0].id;
        let card_id = board
            .add_card(column_id, "参照つきのカード", "")
            .expect("the column takes a card");
        board
            .update_card(
                card_id,
                "参照つきのカード",
                "設計は https://example.com/design 。PR は https://example.com/pull/1",
            )
            .expect("the description is written");
        database
            .save_board(board)
            .expect("the seeded card is stored");
    });
    focus_board(&harness, cx);

    let card_id = harness
        .view
        .read_with(cx, |view, _| {
            view.board.columns[0]
                .cards
                .iter()
                .find(|card| card.title == "参照つきのカード")
                .map(|card| card.id)
        })
        .expect("the seeded card is on the board");

    cx.update(|window, cx| {
        harness
            .view
            .update(cx, |view, cx| view.open_card_panel(card_id, window, cx))
    });
    cx.run_until_parked();

    // 入力欄は本文をそのまま持っている。リンクにするのは描き方の話で、
    // 中身を書き換えてはいない。
    let description = harness.view.read_with(cx, |view, cx| {
        view.editing_card
            .as_ref()
            .expect("the panel is open")
            .description
            .read(cx)
            .value()
            .to_string()
    });
    assert_eq!(
        description, "設計は https://example.com/design 。PR は https://example.com/pull/1",
        "the description is held verbatim, links and all"
    );

    // 打ち直しても保存まで通る。エディタのモードに変えたことで編集経路が
    // 壊れていないことを、画面から見る。パネルを開いた直後はタイトルに
    // フォーカスがあるので、説明へ移してから打つ。
    cx.update(|window, cx| {
        harness.view.update(cx, |view, cx| {
            view.editing_card
                .as_ref()
                .expect("the panel is open")
                .description
                .update(cx, |state, cx| state.focus(window, cx))
        })
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("書き直した説明");
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();

    assert_eq!(
        harness
            .stored_board()
            .columns
            .iter()
            .flat_map(|column| column.cards.iter())
            .find(|card| card.id == card_id)
            .map(|card| card.description.clone()),
        Some("書き直した説明".to_string()),
        "and typing into it still reaches the database"
    );
}
