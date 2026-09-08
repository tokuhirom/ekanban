//! ブラウザだけで動く ekanban（[ADR 0035]）。
//!
//! `crates/app` のコマンドを `wasm32-unknown-unknown` の上で動かし、盤面を
//! **JSON にして `localStorage` に置きます**（`ekanban_core::store::JsonStore`、
//! [ADR 0036]）。答えているのは本物の `ekanban-core` なので、[ADR 0021] の
//! 「偽物のバックエンドを TypeScript で書かない」がブラウザでも成り立ちます。
//!
//! **SQLite はここに積みません。** `wasm32-unknown-unknown` に組んだ SQLite
//! だけで 2.1 MB あり、こちらのコード全部より 5 倍大きい。置き場所を差し替え
//! られるようにしたのはそのためです（[ADR 0036]）。
//!
//! ページからは 2 つだけ見えます。
//!
//! - [`start`]。`localStorage` にあるデータベースを読み込み、起動の状態を返す
//! - [`invoke`]。コマンド名と引数（Tauri に渡すのと同じ camelCase の JSON）を
//!   渡すと、同じ形の答えが返る
//!
//! **ここに盤面の判断を書きません。** 書いた瞬間に 2 つ目の実装ができ、デモの
//! 中でだけ正しいものが生まれます。ここにあるのは環境の差だけです——保存先、
//! ファイルの持ち出し方、URL の開き方。
//!
//! [ADR 0021]: ../../../docs/adr/0021-two-layer-testing-for-the-webview.md
//! [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
//! [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md

// ネイティブでは中身を持ちません。ワークスペースの `cargo build` / `cargo test`
// は、ここを空のライブラリとして通ります。
#![cfg(target_family = "wasm")]

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use ekanban_app::commands;
use ekanban_app::error::{AppError, ErrorKind};
use ekanban_app::state::Source;
use ekanban_app::{dispatch, menu, AppState, Platform, QuickCaptureStatus};
use ekanban_core::store::JsonStore;
use serde::Deserialize;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

/// `localStorage` の鍵。
///
/// **中身は JSON です**（[ADR 0036]）。前の版は SQLite のファイルを base64 に
/// して同じ鍵に置いていたので、そちらは読めません——読めないものは捨てて
/// 新しい盤面から始めます（[`restore`]）。
///
/// [ADR 0036]: ../../../docs/adr/0036-one-model-two-places-to-put-it.md
const STORAGE_KEY: &str = "ekanban:board";

/// 画面に出すデータベースの居場所。
///
/// **パスではありません。** ブラウザにファイルシステムの居場所は無いので、
/// 「どこにあるか」を人が読める形で答えます。`database_location` と
/// `StartupState.database_path` の両方でこれを返します。
const DATABASE_LOCATION: &str = "このブラウザの localStorage";

thread_local! {
    /// 置き場所。ページに 1 つです。`AppState` と、書き出すときの両方から見ます。
    static STORE: RefCell<Option<Arc<Mutex<JsonStore>>>> = const { RefCell::new(None) };
    /// 開いている盤面。ページに 1 つです。
    ///
    /// wasm はシングルスレッドなので、`AppState` の中の `Mutex` が競ることは
    /// ありません。取り合いを防ぐためではなく、**同じ `AppState` を使うため**に
    /// ここに置いてあります。
    static STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
    /// 最後に `localStorage` へ書いた中身。同じものを書き直さないため。
    static SAVED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// 起動する。`StartupState` の JSON を返す。
///
/// `platform` はページが名乗る OS（`"macos"` / `"windows"` / `"linux"`）です。
/// **ブラウザ向けの組み立てでだけ、これを外から受け取ります**——`wasm32-unknown-unknown`
/// は macOS でも Linux でもないので、`Platform::current()` に訊いても
/// キーの割り当てを決められません（[ADR 0035]）。配るアプリのほうは今までどおり
/// Rust がコンパイル時に知っています。
///
/// [ADR 0035]: ../../../docs/adr/0035-a-browser-build-of-the-real-core.md
#[wasm_bindgen]
pub fn start(platform: &str) -> Result<JsValue, JsValue> {
    // パニックが「unreachable executed」だけになると、原因を追う手段が無くなる。
    console_error_panic_hook::set_once();

    let store = Arc::new(Mutex::new(restore()));
    STORE.with_borrow_mut(|slot| *slot = Some(Arc::clone(&store)));

    let source = Source::Json {
        store,
        place: DATABASE_LOCATION.to_string(),
    };
    let (state, mut startup) = commands::load_startup_state(source).map_err(to_js)?;
    startup.platform = parse_platform(platform);

    STATE.with_borrow_mut(|slot| *slot = Some(state));
    // 種を蒔いたばかりのデータベースを、この時点で 1 度書いておく。ここで
    // 落とすと、次に開いたときに空から始まります。
    save()?;

    to_value(&startup)
}

/// コマンドを呼ぶ。引数も答えも、Tauri に渡すのと同じ形の JSON。
///
/// 失敗は `Err` で返します。`AppError` をそのまま積むので、画面の側は Tauri の
/// `invoke` と同じ分岐で受けられます（`web/src/ipc/index.ts`）。
#[wasm_bindgen]
pub fn invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue> {
    let args: Value = if args.is_undefined() || args.is_null() {
        json!({})
    } else {
        serde_wasm_json(&args)?
    };

    let answer = STATE.with_borrow(|slot| {
        let Some(state) = slot.as_ref() else {
            return Err(AppError::new(
                ErrorKind::BoardIo,
                "まだ起動していません",
                "start() を先に呼んでください",
            ));
        };
        host(command, args, state)
    });

    let value = answer.map_err(to_js)?;
    // 盤面を変えたかもしれないなら、写しを取り直す。**既定は「取り直す」**
    // です——読むだけのコマンドを数え落としても、余分に書くだけで済みます。
    if !READ_ONLY.contains(&command) {
        save()?;
    }
    to_value(&value)
}

/// 読むだけのコマンド。写しを取り直す必要がありません。
///
/// **`startup_state` はここに入りません。** 何も入っていなければ最初の盤面を
/// 蒔き、開いていたボードを覚え直すので、それは書き込みです。
const READ_ONLY: &[&str] = &[
    "load_documents",
    "database_location",
    "capture_target",
    "quick_capture_status",
    "export_board_json_contents",
    "stored_board",
    "menu_sections",
    "open_url",
    "log_frontend_error",
];

/// この環境が引き受けるコマンド。残りは共有の表（`ekanban_app::dispatch`）へ。
///
/// **ここに並ぶのは「ブラウザにそれがあるか」の話だけ**です。盤面に関わる
/// ものを 1 つでも足したら、それは 2 つ目の実装になります。
fn host(command: &str, args: Value, state: &AppState) -> Result<Value, AppError> {
    match command {
        // ブラウザに OS の保存ダイアログはありません。**選ぶところが無い**ので、
        // 既定の名前をそのまま返し、書き出しの経路はそのまま通します。実際に
        // 受け取るのはページの「ダウンロード」です。
        "choose_save_path" => ok(read::<FileName>(args)?.file_name),
        // 書き出す JSON。**ファイルに書きません**——ブラウザにファイルシステムが
        // 無いので、中身を返してページに渡します。組み立てるのが置き場所の側なの
        // は、カードの履歴まで入るからです（[ADR 0045]）。Markdown はページが
        // 自分で組み立てるので、ここには来ません。
        //
        // [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md
        "export_board_json_contents" => ok(commands::export_board_json_contents(
            state,
            read::<ExportBoard>(args)?.board_id,
        )?),
        // 盤面まるごとの控え。**SQLite のファイルではありません**（[ADR 0036]）
        // ——置いてあるのが JSON なので、そのままページに渡します。
        "stored_board" => ok(encoded_store()?),
        "database_location" => ok(DATABASE_LOCATION),
        // ファイル管理を開く相手がいません。メニューでは灰色にしてあるので、
        // ここには届かない見込みですが、届いても何も起きないようにします。
        "reveal_path" | "reveal_database" | "reveal_backups" => ok(()),
        // **開いてよい URL かどうかは Rust が決めます**（`docs/DESIGN.md`）。
        // 開くのはページ。断ったときは `null` を返し、何も言いません。
        "open_url" => {
            let url = read::<Url>(args)?.url;
            ok(commands::openable_url(&url))
        }
        // ページの外まで届くキーの割り当ては作れません。理由をそのまま返すと、
        // 割り当てのダイアログが「使えない」と出せます。
        "quick_capture_status" => ok(QuickCaptureStatus {
            unavailable: ekanban_app::shortcut::platform_support().err(),
            failure: None,
        }),
        // メニューバーをページが描くための構成（[ADR 0035]）。**メニューの
        // 中身を決めるのは `menu::sections_for`** で、ここは運ぶだけです。
        "menu_sections" => ok(menu::web_sections(read::<PlatformArg>(args)?.platform)),
        _ => dispatch::invoke(command, args, state)
            .unwrap_or_else(|| Err(dispatch::unknown_command(command, "ブラウザ版"))),
    }
}

// ---------------------------------------------------------------- 保存

/// `localStorage` にあった盤面。読めなければ空の置き場所。
///
/// **読めなくても起動を止めません。** 読めない文字列を抱えて起動を断ると、
/// ページを開くことすらできなくなります。前の版が置いた SQLite の base64 も
/// ここで捨てられます。
fn restore() -> JsonStore {
    storage()
        .ok()
        .and_then(|storage| storage.get_item(STORAGE_KEY).ok().flatten())
        .and_then(|stored| JsonStore::decode(&stored))
        .unwrap_or_default()
}

/// いまの盤面を `localStorage` に写す。
///
/// **書けなかったことを黙って飲み込みません。** 置き場所が一杯になったら
/// （`localStorage` は数 MB で埋まります）、次に開いたときに変更が消えます。
/// 気づけるのはその場だけなので、失敗をそのまま画面へ返します。
fn save() -> Result<(), JsValue> {
    let encoded = encoded_store().map_err(to_js)?;
    let unchanged = SAVED.with_borrow(|saved| saved.as_deref() == Some(encoded.as_str()));
    if unchanged {
        return Ok(());
    }
    storage()
        .map_err(to_js)?
        .set_item(STORAGE_KEY, &encoded)
        .map_err(|_| {
            to_js(storage_error(
                "保存できませんでした",
                "ブラウザの保存領域が一杯です。書き出してから、いらないボードを消してください",
            ))
        })?;
    SAVED.with_borrow_mut(|saved| *saved = Some(encoded));
    Ok(())
}

/// 置いてある形の文字列。持ち出し（「盤面をコピー…」）にも使います。
fn encoded_store() -> Result<String, AppError> {
    STORE.with_borrow(|slot: &Option<Arc<Mutex<JsonStore>>>| {
        let Some(store) = slot.as_ref() else {
            return Err(storage_error(
                "保存できませんでした",
                "まだ起動していません",
            ));
        };
        store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .encode()
            .map_err(|error| storage_error("保存できませんでした", &error.to_string()))
    })
}

fn storage() -> Result<web_sys::Storage, AppError> {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .ok_or_else(|| {
            storage_error(
                "保存できませんでした",
                "このブラウザでは localStorage を使えません",
            )
        })
}

fn storage_error(message: &str, detail: &str) -> AppError {
    AppError::new(ErrorKind::BoardIo, message, detail)
}

// ---------------------------------------------------------------- 受け渡し

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

/// JavaScript の値を `serde_json::Value` にする。
///
/// `JSON.stringify` を通します。**数値の扱いを Tauri と同じにするため**で、
/// ここを独自に変換すると、`i64` の境目（`docs/DESIGN.md`「境界を越える値」）で
/// ハーネスと答えが変わります。
fn serde_wasm_json(value: &JsValue) -> Result<Value, JsValue> {
    let text = js_sys::JSON::stringify(value)
        .map_err(|_| to_js(read_failed("引数を JSON にできませんでした")))?;
    serde_json::from_str(&String::from(text))
        .map_err(|error| to_js(read_failed(&error.to_string())))
}

fn read_failed(detail: &str) -> AppError {
    AppError::new(ErrorKind::Validation, "引数を読めませんでした", detail)
}

/// 答えを JavaScript の値にする。`JSON.parse` を通すのは [`serde_wasm_json`] と同じ理由。
fn to_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let text = serde_json::to_string(value)
        .map_err(|error| JsValue::from_str(&format!("答えを組み立てられません: {error}")))?;
    js_sys::JSON::parse(&text)
}

/// `AppError` を、Tauri の `invoke` が reject するのと同じ形で積む。
fn to_js(error: AppError) -> JsValue {
    serde_json::to_string(&error)
        .ok()
        .and_then(|text| js_sys::JSON::parse(&text).ok())
        .unwrap_or_else(|| JsValue::from_str("不明な失敗"))
}

fn parse_platform(platform: &str) -> Platform {
    match platform {
        "macos" => Platform::Macos,
        "windows" => Platform::Windows,
        _ => Platform::Linux,
    }
}

// ---------------------------------------------------------------- 引数の形

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileName {
    file_name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportBoard {
    board_id: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Url {
    url: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformArg {
    platform: Platform,
}
