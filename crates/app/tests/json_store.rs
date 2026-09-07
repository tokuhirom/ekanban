//! 同じコマンドを、JSON の置き場所に対して通す（[ADR 0036]）。
//!
//! `commands.rs` のテストは SQLite を相手にしています。ここが見るのは
//! **置き場所を差し替えても同じことが起きるか**で、盤面の振る舞いを数え直しは
//! しません——動いているのは同じ `model.rs` だからです。
//!
//! 見るのは 3 つです。
//!
//! 1. **置き場所を 2 度開かないこと。** SQLite は接続を 2 つ持てるので気づけ
//!    ませんが、JSON の置き場所はページに 1 つで、同じ `Mutex` を 2 度取ると
//!    落ちます。起動と、クイックキャプチャがそこを通ります
//! 2. **書いたものが読み戻せること。** 文字列にして持ち歩ける形で残り、
//!    採番の続き（`next_card_id` など）まで戻ること
//! 3. **付随する表示の状態が、SQLite と同じ鍵で残ること**
//!
//! [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md

use std::sync::{Arc, Mutex};

use ekanban_app::commands;
use ekanban_app::snapshot::ThemePreference;
use ekanban_app::state::Source;
use ekanban_app::AppState;
use ekanban_core::store::{FilterState, JsonStore};

const PLACE: &str = "テストの中";

fn json_source(store: JsonStore) -> Source {
    Source::Json {
        store: Arc::new(Mutex::new(store)),
        place: PLACE.to_string(),
    }
}

/// 空の置き場所から起動する。種を蒔いた盤面が入っている。
fn open() -> (AppState, Source) {
    let source = json_source(JsonStore::new());
    let (state, startup) =
        commands::load_startup_state(source.clone()).expect("the startup state is read");
    assert_eq!(startup.database_path, PLACE);
    (state, source)
}

/// いま置き場所に入っている文字列。**ページが `localStorage` に置くのと同じもの。**
fn encoded(source: &Source) -> String {
    let Source::Json { store, .. } = source else {
        unreachable!("この試験は JSON の置き場所だけを見る")
    };
    store
        .lock()
        .expect("the store is not poisoned")
        .encode()
        .expect("the store encodes")
}

/// 起動が、置き場所を 2 度開かずに済むこと。
///
/// **ここが落ちるのは「recursively acquire mutex」で**、SQLite では起きません
/// （接続を 2 つ開けるので）。`startup_state` が持っている置き場所から
/// スナップショットを組んでいるかどうかを見ています。
#[test]
fn starting_up_opens_the_store_once() {
    let (_state, source) = open();
    // 種を蒔いた盤面が、そのまま持ち出せる形で入っていること。
    assert!(encoded(&source).contains("やること"));
}

/// クイックキャプチャも、置き場所を 2 度開かない。
#[test]
fn quick_capture_opens_the_store_once() {
    let (state, source) = open();
    let snapshot = commands::capture_card(&state, "牛乳を買う").expect("the card is captured");
    assert!(snapshot
        .board
        .columns
        .iter()
        .flat_map(|column| column.cards.iter())
        .any(|card| card.title == "牛乳を買う"));
    assert!(encoded(&source).contains("牛乳を買う"));
}

/// 書いたものが、文字列を経由して読み戻せること。
///
/// **採番の続きまで戻ること**を見ます。`Board` の `next_*_id` は
/// `#[serde(skip)]` なので、そのまま `serde_json` に渡すと落ちます。落ちると、
/// 次に開いたときに同じ ID を振り直します。
#[test]
fn a_board_survives_a_round_trip_through_the_stored_string() {
    let (state, source) = open();
    let column = state.snapshot().expect("a snapshot").board.columns[0].id;
    commands::add_card(&state, column, "1 枚目", "", "", Vec::new(), Vec::new())
        .expect("the card is added");
    commands::add_card(&state, column, "2 枚目", "", "", Vec::new(), Vec::new())
        .expect("the card is added");
    let before = state.snapshot().expect("a snapshot").board;

    // ページを開き直したのと同じこと。
    let stored = encoded(&source);
    let restored = JsonStore::decode(&stored).expect("the stored string reads back");
    let (reopened, _) =
        commands::load_startup_state(json_source(restored)).expect("the startup state is read");
    let after = reopened.snapshot().expect("a snapshot").board;

    assert_eq!(after, before, "読み戻した盤面が元と違う");

    // 続きから振れること。同じ ID を振り直したら、ここで衝突する。
    let ids: Vec<_> = after
        .columns
        .iter()
        .flat_map(|column| column.cards.iter().map(|card| card.id))
        .collect();
    let snapshot = commands::add_card(&reopened, column, "3 枚目", "", "", Vec::new(), Vec::new())
        .expect("the card is added");
    let added = snapshot.board.columns[0]
        .cards
        .iter()
        .find(|card| card.title == "3 枚目")
        .expect("the new card is there");
    assert!(!ids.contains(&added.id), "採番の続きが戻っていない");
}

/// 付随する表示の状態も残ること。
#[test]
fn the_display_state_survives_a_round_trip() {
    let (state, source) = open();
    commands::set_theme_preference(&state, ThemePreference::Dark).expect("the theme is stored");
    commands::set_sidebar_collapsed(&state, true).expect("the sidebar state is stored");
    commands::set_filter_state(
        &state,
        &FilterState {
            search: "牛乳".to_string(),
            tag_id: None,
        },
    )
    .expect("the filter is stored");

    let restored = JsonStore::decode(&encoded(&source)).expect("the stored string reads back");
    let (_, startup) =
        commands::load_startup_state(json_source(restored)).expect("the startup state is read");

    assert_eq!(startup.theme, ThemePreference::Dark);
    assert!(startup.sidebar_collapsed);
    assert_eq!(startup.filter.search, "牛乳");
}

/// ボードを足して切り替えても、ID がぶつからないこと。
///
/// ID は主キー 1 本なので、ボードごとに区画を取ります（`store::board_scoped_id`）。
/// SQLite 側と同じ規則を JSON 側も通ることを見ます。
#[test]
fn boards_get_their_own_id_namespace() {
    let (state, _source) = open();
    let first = state.snapshot().expect("a snapshot").board.id;
    let snapshot = commands::create_board(&state, "2 つめ").expect("the board is created");
    assert_ne!(snapshot.board.id, first);
    assert_eq!(snapshot.boards.len(), 2);

    let column = snapshot.board.columns[0].id;
    let added = commands::add_card(&state, column, "別のボードのカード", "", "", vec![], vec![])
        .expect("the card is added");
    let new_card = added.board.columns[0].cards[0].id;

    let back = commands::switch_board(&state, first).expect("the board switches");
    let old_ids: Vec<_> = back
        .board
        .columns
        .iter()
        .flat_map(|column| column.cards.iter().map(|card| card.id))
        .collect();
    assert!(
        !old_ids.contains(&new_card),
        "ボードをまたいで ID が重なった"
    );
}

/// 読めない文字列は、起動を止めずに捨てられること。
#[test]
fn an_unreadable_stored_string_is_dropped() {
    // 前の版が置いた SQLite の base64 も、ここで捨てられます。
    assert!(JsonStore::decode("これは JSON ではない").is_none());
}
