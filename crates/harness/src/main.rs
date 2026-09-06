//! `crates/app` のコマンドを HTTP に出す、開発とテスト専用のバイナリ。
//!
//! ```sh
//! cargo run -p ekanban-harness -- /tmp/board.sqlite3 [ポート]
//! ```
//!
//! `POST /invoke/<コマンド名>` に、Tauri へ渡すのと同じ形（camelCase の JSON）
//! で引数を送ると、同じ形の答えが返ります。webview 側は `web/src/ipc/` の口を
//! 差し替えるだけで、同じ画面がふつうのブラウザで動きます
//! （`docs/TAURI-MIGRATION.md` §10）。
//!
//! **偽物のバックエンドを TypeScript で書かないため**にあります（ADR 0021）。
//! 通っているのは本物の `ekanban-core` なので、モデルの挙動がテストの中でだけ
//! 違う、が起きません。
//!
//! 配りません。`127.0.0.1` にだけ結び、認証も暗号化も持ちません。

use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;

use ekanban_app::commands;
use ekanban_app::error::{AppError, ErrorKind};
use ekanban_app::AppState;
use serde::Deserialize;
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(database_path) = args.next().map(PathBuf::from) else {
        eprintln!("使い方: ekanban-harness <データベースのパス> [ポート]");
        std::process::exit(2);
    };
    let port: u16 = args.next().map_or(1421, |value| {
        value.parse().expect("ポートは数値で渡してください")
    });

    if let Some(parent) = database_path.parent() {
        std::fs::create_dir_all(parent).expect("データベースの置き場所を作れません");
    }
    let (state, _) = commands::load_startup_state(&database_path).unwrap_or_else(|error| {
        eprintln!(
            "{} を開けませんでした: {}",
            database_path.display(),
            error.detail
        );
        std::process::exit(1);
    });

    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let server = Server::http(address).expect("ポートを開けません");
    // 立ち上がったことを 1 行で知らせる。テストはこれを待って動きはじめる。
    println!(
        "ekanban-harness listening on http://{address} ({})",
        database_path.display()
    );

    for request in server.incoming_requests() {
        handle(request, &state);
    }
}

fn handle(mut request: Request, state: &AppState) {
    // ブラウザは開発サーバ（別のポート）から呼ぶので、事前確認が飛んでくる。
    if request.method() == &Method::Options {
        let _ = request.respond(cors(Response::empty(204)));
        return;
    }

    let Some(command) = request.url().strip_prefix("/invoke/").map(str::to_owned) else {
        let _ = request.respond(cors(
            Response::from_string("not found").with_status_code(404),
        ));
        return;
    };

    let mut body = String::new();
    if std::io::Read::read_to_string(request.as_reader(), &mut body).is_err() {
        let _ = request.respond(cors(
            Response::from_string("bad body").with_status_code(400),
        ));
        return;
    }
    let args: Value = if body.trim().is_empty() {
        json!({})
    } else {
        match serde_json::from_str(&body) {
            Ok(value) => value,
            Err(error) => {
                let _ = request.respond(cors(
                    Response::from_string(error.to_string()).with_status_code(400),
                ));
                return;
            }
        }
    };

    let (status, payload) = match invoke(&command, args, state) {
        Ok(value) => (200, value),
        Err(error) => (
            400,
            serde_json::to_value(&error).unwrap_or_else(|_| json!({ "detail": "不明な失敗" })),
        ),
    };
    let response = Response::from_string(payload.to_string())
        .with_status_code(status)
        .with_header(header("Content-Type", "application/json"));
    let _ = request.respond(cors(response));
}

/// コマンド名で振り分ける。**名前は Tauri 側（`crates/app/src/ipc.rs`）と同じ**。
///
/// 引数の読み取り以外のことをここに書かないでください。判断が入りはじめたら、
/// それは Tauri 側とハーネスで違う動きになるということです。
fn invoke(command: &str, args: Value, state: &AppState) -> Result<Value, AppError> {
    fn ok<T: serde::Serialize>(value: T) -> Result<Value, AppError> {
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

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct BoardId {
        board_id: i64,
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
    struct FilterState {
        filter: ekanban_core::db::FilterState,
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

    match command {
        "startup_state" => ok(commands::startup_state(state)?),
        "snapshot" => ok(state.snapshot()?),
        "switch_board" => ok(commands::switch_board(
            state,
            read::<BoardId>(args)?.board_id,
        )?),
        "move_card" => {
            let a: MoveCard = read(args)?;
            ok(commands::move_card(
                state,
                a.card_id,
                a.to_column_id,
                a.to_index,
            )?)
        }
        "move_column" => {
            let a: MoveColumn = read(args)?;
            ok(commands::move_column(state, a.column_id, a.to_index)?)
        }
        "filter_cards" => {
            let a: Filter = read(args)?;
            ok(commands::filter_cards(state, &a.query, a.tag_id))
        }
        "set_filter_state" => ok(commands::set_filter_state(
            state,
            &read::<FilterState>(args)?.filter,
        )?),
        "set_sidebar_collapsed" => ok(commands::set_sidebar_collapsed(
            state,
            read::<Collapsed>(args)?.collapsed,
        )?),
        "log_frontend_error" => {
            commands::log_frontend_error(&read::<Message>(args)?.message);
            ok(())
        }
        _ => Err(AppError::new(
            ErrorKind::BoardIo,
            "知らないコマンドです",
            format!("{command} は ekanban-harness に出ていません"),
        )),
    }
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("固定の値なので必ず作れる")
}

/// 開発サーバ（別のポート）から呼べるようにする。
///
/// `127.0.0.1` にだけ結んであり、配らないので、ここを絞る意味がありません。
fn cors<R>(response: Response<R>) -> Response<R>
where
    R: std::io::Read,
{
    response
        .with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Headers", "Content-Type"))
        .with_header(header("Access-Control-Allow-Methods", "POST, OPTIONS"))
}
