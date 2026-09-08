//! `crates/app` のコマンドを HTTP に出す、開発とテスト専用のバイナリ。
//!
//! ```sh
//! cargo run -p ekanban-harness -- /tmp/board.sqlite3 [ポート]
//! ```
//!
//! `POST /invoke/<コマンド名>` に、Tauri へ渡すのと同じ形（camelCase の JSON）
//! で引数を送ると、同じ形の答えが返ります。webview 側は `web/src/ipc/` の口を
//! 差し替えるだけで、同じ画面がふつうのブラウザで動きます
//! （`docs/DESIGN.md`「テスト」）。
//!
//! **偽物のバックエンドを TypeScript で書かないため**にあります（ADR 0021）。
//! 通っているのは本物の `ekanban-core` なので、モデルの挙動がテストの中でだけ
//! 違う、が起きません。
//!
//! 配りません。`127.0.0.1` にだけ結び、認証も暗号化も持ちません。

use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;

use ekanban_app::commands;
use ekanban_app::dispatch;
use ekanban_app::error::{AppError, ErrorKind};
use ekanban_app::shortcut::{KeyPress, Shortcut};
use ekanban_app::state::Source;
use ekanban_app::{AppState, QuickCaptureStatus};
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
    let (state, _) = commands::load_startup_state(Source::Sqlite(database_path.clone()))
        .unwrap_or_else(|error| {
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

/// コマンド名で振り分ける。
///
/// **盤面に関わるものは `ekanban_app::dispatch` が引き受けます**——表を 2 つ
/// 持たないためで、ブラウザ向けの組み立て（`crates/web`、[ADR 0035]）も同じ表を
/// 通ります。ここに残っているのは、**環境で答えが変わるものだけ**です。
///
/// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
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
    struct FileName {
        file_name: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TextFile {
        destination: PathBuf,
        extension: String,
        contents: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Destination {
        destination: PathBuf,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ExportBoard {
        board_id: i64,
        destination: PathBuf,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Url {
        url: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Press {
        press: Option<KeyPress>,
    }

    match command {
        // ブラウザに OS の保存ダイアログはありません。**選ぶところだけ**を
        // データベースの隣に決め打ちで返し、書き出しの経路はそのまま通します。
        // 開発とテストのためのもので、配るものには入りません。
        "choose_save_path" => {
            let file_name = read::<FileName>(args)?.file_name;
            let directory = commands::database_location(state)
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            ok(directory.join(file_name))
        }
        "export_board_json" => {
            let a: ExportBoard = read(args)?;
            ok(commands::export_board_json(
                state,
                a.board_id,
                &a.destination,
            )?)
        }
        "write_text_file" => {
            let a: TextFile = read(args)?;
            ok(commands::write_text_file(
                &a.destination,
                &a.extension,
                &a.contents,
            )?)
        }
        "backup_database" => ok(commands::backup_database(
            state,
            &read::<Destination>(args)?.destination,
        )?),
        "database_location" => ok(commands::database_location(state)),
        // 場所を開く相手（OS のファイル管理）がブラウザにはいない。押しても
        // 何も起きないことだけが本物と違う。
        "reveal_path" | "reveal_database" | "reveal_backups" => ok(()),
        // ブラウザにグローバルホットキーはありません。登録できるかどうかは
        // 本物の窓の話なので、ここでは「使える」ことにして、割り当ての読み取りと
        // 保存だけを本物と同じ経路に通します。
        "quick_capture_status" => ok(QuickCaptureStatus {
            unavailable: None,
            failure: None,
        }),
        "set_quick_capture_shortcut" => {
            let press = read::<Press>(args)?.press;
            let stored = match press {
                Some(press) => Some(
                    Shortcut::from_key_press(&press)
                        .map_err(|error| {
                            AppError::new(
                                ErrorKind::Shortcut,
                                "ショートカットを割り当てられません",
                                error.to_string(),
                            )
                        })?
                        .to_string(),
                ),
                None => None,
            };
            commands::set_quick_capture_shortcut(state, stored.as_deref())?;
            ok(stored)
        }
        // 開く先のブラウザが、すでにブラウザ。記録だけ残す。
        "open_url" => {
            let url = read::<Url>(args)?.url;
            match commands::openable_url(&url) {
                Some(url) => println!("open_url {url}"),
                None => println!("refused to open {url}"),
            }
            ok(())
        }
        _ => dispatch::invoke(command, args, state)
            .unwrap_or_else(|| Err(dispatch::unknown_command(command, "ekanban-harness"))),
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
