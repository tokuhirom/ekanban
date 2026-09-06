//! マニュアル（`docs/MANUAL.md`）のスクリーンショット用のデータベースを作る。
//!
//! `script/manual-screenshots` から 1 枚ごとに呼ばれる。撮りたい画面の名前を渡すと
//! `EKANBAN_DATABASE` のデータベースを作り直し、その画面が復元されるところまで
//! 保存する。アプリを起動すればそのまま撮れる状態になる。
//!
//! ```sh
//! EKANBAN_DATABASE=/tmp/manual.sqlite3 cargo run --example manual_screenshot_seed -- board-dark
//! ```
//!
//! 盤面は SQL ではなくアプリ自身の API で組み立てる。撮った画面が、アプリが本当に
//! 復元できる状態であることを、作り方の側で保証するため。

use chrono::{Duration, Local, NaiveDate};
use ekanban::db::{Database, FilterState};
use ekanban::model::{Board, CardId, ColumnId, TagId};

/// 撮れる画面の名前と、それが何を見せているか。
const SCREENS: &[(&str, &str)] = &[
    ("board", "ボードの全体"),
    ("card-edit", "カードの編集パネル"),
    ("search", "検索での絞り込み"),
    ("filter-tag", "タグでの絞り込み"),
    ("board-list-collapsed", "ボード一覧を畳んだところ"),
    ("board-dark", "ダークモード"),
];

/// 検索の絞り込みに使う語。`script/manual-screenshots` の説明と揃える。
const SEARCH_TERM: &str = "SQLite";

fn main() {
    let screen = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "board".to_string());
    if !SCREENS.iter().any(|(name, _)| *name == screen) {
        eprintln!("撮れる画面の名前ではありません: {screen}");
        eprintln!("次のどれかを渡してください:");
        for (name, description) in SCREENS {
            eprintln!("  {name:<21} {description}");
        }
        std::process::exit(2);
    }

    let path = ekanban::database_path();
    if let Err(error) = std::fs::remove_file(&path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("{} を消せませんでした: {error}", path.display());
            std::process::exit(1);
        }
    }

    // 空のデータベースを開くと、デモのボードが 1 つ入った状態で作られる。
    // それを撮りたい盤面に組み替える。
    let mut database = Database::open(&path).expect("空のデータベースを作れる");
    let mut board = database.load_board().expect("デモのボードがある");
    let tags = build_personal_board(&mut board);
    database.save_board(&mut board).expect("ボードを保存できる");

    let mut home = database.create_board("家のこと").expect("ボードを作れる");
    build_home_board(&mut home);
    database.save_board(&mut home).expect("ボードを保存できる");

    database
        .set_capture_target(Some((board.id, board.columns[0].id)))
        .expect("キャプチャ先を保存できる");
    database
        .set_last_board_id(board.id)
        .expect("開くボードを保存できる");

    // ここから下が画面ごとの違い。既定は「何も絞り込んでいないライトモード」で、
    // board と card-edit はそのまま撮る（card-edit のパネルは保存されないので、
    // 撮る側でカードを押して開く）。
    let filter = match screen.as_str() {
        "search" => FilterState {
            search: SEARCH_TERM.to_string(),
            tag_id: None,
        },
        "filter-tag" => FilterState {
            search: String::new(),
            tag_id: Some(tags.deadline),
        },
        _ => FilterState::default(),
    };
    database
        .set_filter_state(&filter)
        .expect("絞り込みを保存できる");
    database
        .set_sidebar_collapsed(screen == "board-list-collapsed")
        .expect("ボード一覧の状態を保存できる");
    database
        .set_theme_preference(if screen == "board-dark" {
            "dark"
        } else {
            "light"
        })
        .expect("テーマを保存できる");
}

/// 撮影用に作るタグ。
struct Tags {
    design: TagId,
    research: TagId,
    deadline: TagId,
}

/// 「個人 Kanban」を組み立てる。
///
/// 期限は撮る日からの日数で決める。マニュアルの「期限の書き分け」の表と揃うよう、
/// 期限切れ・今日・近い・それより先が 1 枚ずつ出るようにしてある。
fn build_personal_board(board: &mut Board) -> Tags {
    let tags = Tags {
        design: board.add_tag("設計", "#8b5cf6").expect("新しいタグ"),
        research: board.add_tag("調査", "#22c55e").expect("新しいタグ"),
        deadline: board.add_tag("締切あり", "#ef4444").expect("新しいタグ"),
    };

    let todo = board.columns[0].id;
    let doing = board.columns[1].id;
    let done = board.columns[2].id;
    let later = board.add_column("寝かせる").expect("新しいカラム");
    // WIP 上限に達したカラムを 1 つ見せる。
    board
        .set_column_wip_limit(doing, Some(2))
        .expect("カラムがある");

    // 初回のシード（`Board::first_run`）が作るのは空の 3 カラムだけなので、撮る
    // 盤面のカードはここで全部足す。中身は下の `edit` で埋める。
    let demo_todo = [
        add(board, todo, "GPUI の画面を作る", ""),
        add(board, todo, "ドラッグ＆ドロップを試す", ""),
    ];
    let demo_doing = add(board, doing, "SQLite のスキーマを決める", "");
    let demo_done = add(board, done, "README を書く", "");

    edit(
        board,
        demo_todo[0],
        "GPUI の画面を作る",
        "カラムとカードの描画をひととおり通す。",
        day(-2),
        vec![tags.design],
    );
    edit(
        board,
        demo_todo[1],
        "ドラッグ＆ドロップを試す",
        "カラム間の移動と、カラム内の並べ替え。",
        day(3),
        vec![tags.research],
    );
    let review = add(
        board,
        todo,
        "週次の振り返りを書く",
        "今週やったことを 10 行でまとめる。",
    );
    edit(
        board,
        review,
        "週次の振り返りを書く",
        "今週やったことを 10 行でまとめる。",
        day(6),
        vec![tags.deadline],
    );
    add(
        board,
        todo,
        "色のコントラストを確認する",
        "ライトとダークの両方で読めるか。",
    );

    // 編集パネルの screenshot はこのカードを開いて撮る。期限・タグ・チェックリスト
    // が全部埋まっている 1 枚が要るのは、パネルの項目をひととおり見せるため。
    edit(
        board,
        demo_doing,
        "SQLite のスキーマを決める",
        "差分保存と UPSERT の形を先に決める。",
        day(1),
        vec![tags.design, tags.deadline],
    );
    for (text, checked) in [
        ("テーブルの列を洗い出す", true),
        ("移行の手順を決める", false),
        ("round-trip のテストを書く", false),
    ] {
        let item = board
            .add_checklist_item(demo_doing, text)
            .expect("カードがある");
        if checked {
            board
                .set_checklist_item_checked(demo_doing, item, true)
                .expect("項目がある");
        }
    }

    let keyboard = add(
        board,
        doing,
        "キーボード操作を詰める",
        "矢印で選び、Ctrl+Alt+矢印で動かす。",
    );
    edit(
        board,
        keyboard,
        "キーボード操作を詰める",
        "矢印で選び、Ctrl+Alt+矢印で動かす。",
        day(0),
        vec![tags.design],
    );

    edit(
        board,
        demo_done,
        "README を書く",
        "何ができるアプリなのかを 1 段落で。",
        None,
        vec![],
    );
    let url = add(
        board,
        later,
        "URL スキーマの案をためる",
        "起動中の 1 つに渡す仕組みが要る。",
    );
    edit(
        board,
        url,
        "URL スキーマの案をためる",
        "起動中の 1 つに渡す仕組みが要る。",
        None,
        vec![tags.research],
    );

    tags
}

/// 「家のこと」を組み立てる。
///
/// 開いた画面は撮らない。ボード一覧に 2 つ目が並んでいるところだけを見せる。
fn build_home_board(board: &mut Board) {
    let errands = board.columns[0].id;
    board.add_column("済み").expect("新しいカラム");
    add(board, errands, "洗剤を買う", "詰め替えの大きいほう。");
    add(board, errands, "自転車の空気を入れる", "");
}

/// 撮る日から `offset` 日ずらした日付。
fn day(offset: i64) -> Option<NaiveDate> {
    Some(Local::now().date_naive() + Duration::days(offset))
}

fn add(board: &mut Board, column_id: ColumnId, title: &str, description: &str) -> CardId {
    board
        .add_card(column_id, title, description)
        .expect("カラムがある")
}

fn edit(
    board: &mut Board,
    card_id: CardId,
    title: &str,
    description: &str,
    due_date: Option<NaiveDate>,
    tag_ids: Vec<TagId>,
) {
    board
        .update_card_details(card_id, title, description, due_date, tag_ids)
        .expect("カードがある");
}
