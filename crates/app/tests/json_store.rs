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
//!    落ちます。起動と、盤面の書き込みがそこを通ります
//! 2. **書いたものが読み戻せること。** 文字列にして持ち歩ける形で残り、
//!    採番の続き（`next_card_id` など）まで戻ること
//! 3. **付随する表示の状態が、SQLite と同じ鍵で残ること**
//!
//! [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md

use std::sync::{Arc, Mutex};

use ekanban_app::commands;
use ekanban_app::commands::BoardDocument;
use ekanban_app::snapshot::ThemePreference;
use ekanban_app::state::Source;
use ekanban_app::AppState;
use ekanban_core::model::Card;
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

/// 先頭のボードを、webview が受け取るのと同じ形で読む。
fn first_document(state: &AppState) -> BoardDocument {
    commands::load_documents(state)
        .expect("the documents load")
        .remove(0)
}

/// webview がやるのと同じことを、こちらで手で当てる。
///
/// **盤面の操作は Rust にありません**（[ADR 0039]）ので、テストも「画面が
/// 当てた盤面」を組み立てて渡します。
///
/// [ADR 0039]: ../../docs/adr/0039-the-board-model-moves-to-typescript.md
fn push_card(document: &mut BoardDocument, title: &str) -> i64 {
    let id = document.next_card_id;
    document.next_card_id += 1;
    let column = &mut document.board.columns[0];
    let position = i64::try_from(column.cards.len()).expect("a card index fits");
    column.cards.push(Card {
        id,
        column_id: column.id,
        title: title.to_string(),
        description: String::new(),
        position,
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
        due_date: None,
        tag_ids: Vec::new(),
        checklist_items: Vec::new(),
        archived_at: None,
    });
    id
}

/// 起動が、置き場所を 2 度開かずに済むこと。
///
/// **ここが落ちるのは「recursively acquire mutex」で**、SQLite では起きません
/// （接続を 2 つ開けるので）。
#[test]
fn starting_up_opens_the_store_once() {
    let (_state, source) = open();
    // 種を蒔いた盤面が、そのまま持ち出せる形で入っていること。
    assert!(encoded(&source).contains("やること"));
}

/// 盤面を書くときも、置き場所を 2 度開かない。
#[test]
fn saving_a_document_opens_the_store_once() {
    let (state, source) = open();
    let mut document = first_document(&state);
    push_card(&mut document, "牛乳を買う");
    commands::save_document(&state, document, Vec::new()).expect("the document is saved");

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
    let mut document = first_document(&state);
    push_card(&mut document, "1 枚目");
    push_card(&mut document, "2 枚目");
    commands::save_document(&state, document, Vec::new()).expect("the document is saved");
    let before = first_document(&state);

    // ページを開き直したのと同じこと。
    let stored = encoded(&source);
    let restored = JsonStore::decode(&stored).expect("the stored string reads back");
    let (reopened, _) =
        commands::load_startup_state(json_source(restored)).expect("the startup state is read");
    let mut after = first_document(&reopened);

    assert_eq!(after.board, before.board, "読み戻した盤面が元と違う");
    assert_eq!(
        after.next_card_id, before.next_card_id,
        "採番の続きが戻っていない"
    );

    // 続きから振れること。同じ ID を振り直したら、ここで衝突する。
    let ids: Vec<_> = after
        .board
        .columns
        .iter()
        .flat_map(|column| column.cards.iter().map(|card| card.id))
        .collect();
    let added = push_card(&mut after, "3 枚目");
    assert!(!ids.contains(&added), "採番の続きが戻っていない");
    commands::save_document(&reopened, after, Vec::new()).expect("the document is saved");
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

/// ボードを足しても、ID がぶつからないこと。
///
/// ID は主キー 1 本なので、ボードごとに区画を取ります（`store::board_scoped_id`）。
/// SQLite 側と同じ規則を JSON 側も通ることを見ます。
#[test]
fn boards_get_their_own_id_namespace() {
    let (state, _source) = open();
    let first = first_document(&state);
    let mut created = commands::create_board(&state, "2 つめ").expect("the board is created");
    assert_ne!(created.board.id, first.board.id);
    assert_eq!(commands::load_documents(&state).unwrap().len(), 2);

    let new_card = push_card(&mut created, "別のボードのカード");
    let old_ids: Vec<_> = first
        .board
        .columns
        .iter()
        .flat_map(|column| column.cards.iter().map(|card| card.id))
        .collect();
    assert!(
        !old_ids.contains(&new_card),
        "ボードをまたいで ID が重なった"
    );
    commands::save_document(&state, created, Vec::new()).expect("the document is saved");
}

/// 読めない文字列は、起動を止めずに捨てられること。
#[test]
fn an_unreadable_stored_string_is_dropped() {
    // 前の版が置いた SQLite の base64 も、ここで捨てられます。
    assert!(JsonStore::decode("これは JSON ではない").is_none());
}

/// 前の版が置いた文字列でも、かつての既定色のタグが自動の色に戻ること
/// （ADR 0044）。SQLite 側の移行 13 と同じことを、ブラウザの置き場所でも行う。
///
/// **`version` の無い文字列を読ませます。** ブラウザに置いてあるのは、まさに
/// その形のままだからです。
#[test]
fn the_old_default_tag_color_becomes_automatic_when_reading_an_older_string() {
    let (state, source) = open();
    let mut document = first_document(&state);
    let board_id = document.board.id;
    for (name, color) in [("既定色のまま", "#94a3b8"), ("自分で選んだ", "#ef4444")] {
        let id = document.next_tag_id;
        document.next_tag_id += 1;
        document.board.tags.push(ekanban_core::model::Tag {
            id,
            board_id,
            name: name.to_string(),
            color: color.to_string(),
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_000_000,
        });
    }
    commands::save_document(&state, document, Vec::new()).expect("the tags are saved");

    // 版の欄を落として、この移行より前に置かれた文字列にする。
    let stored = encoded(&source);
    let mut value: serde_json::Value = serde_json::from_str(&stored).expect("the string is JSON");
    assert!(
        value
            .as_object_mut()
            .expect("the store is an object")
            .remove("version")
            .is_some(),
        "いまの置き場所には版が入っている"
    );
    let older = serde_json::to_string(&value).expect("the older string is written");

    let restored = JsonStore::decode(&older).expect("the older string reads back");
    let (reopened, _) =
        commands::load_startup_state(json_source(restored)).expect("the startup state is read");

    let tags = first_document(&reopened).board.tags;
    let color = |name: &str| {
        tags.iter()
            .find(|tag| tag.name == name)
            .expect("the tag survives")
            .color
            .clone()
    };
    assert_eq!(color("既定色のまま"), "", "既定色は自動の色に戻る");
    assert_eq!(color("自分で選んだ"), "#ef4444", "選んだ色はそのまま");
}
