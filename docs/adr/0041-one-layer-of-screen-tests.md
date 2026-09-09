# 0041. 画面のテストを 1 層にし、ハーネスを畳む

- 日付: 2026-09-08
- 状態: 有効
- 関連: [0021](0021-two-layer-testing-for-the-webview.md)（これを置き換える）、[0023](0023-verifying-the-webview-engines.md)、[0039](0039-the-board-model-moves-to-typescript.md)

## 状況

[0021](0021-two-layer-testing-for-the-webview.md) は「**偽物のバックエンドを TypeScript で書かない**」ために `ekanban-harness` を作った。`crates/app` のコマンドを同じ名前で HTTP に出し、Playwright が本物の `ekanban-core` を相手にする。`web/e2e/harness.ts` はテストごとにデータベースとハーネスを 1 つずつ立て、`invoke()` で盤面を読み戻して「画面と SQLite の両方」を見る。

[0039](0039-the-board-model-moves-to-typescript.md) で盤面のモデルが TypeScript に移ると、**Playwright が動かすのは本物のモデルそのものになる**。偽物を書く余地が無くなり、HTTP に出す相手もいなくなる。ハーネスは、守るものが無くなった層として残る。

## 決定

**`crates/harness` を畳み、画面のテストを 1 層にする。**

- `web/e2e/` は Vite の開発サーバの上で本物の画面を開き、置き場所には `store/memory`（テストごとに空）を差す。テストは置き場所を直接読み戻して、**画面と保存の両方**を見る——見るものは [0021](0021-two-layer-testing-for-the-webview.md) と同じで、読み戻す先が SQLite からメモリの文書に変わる
- **SQLite に届くことは Rust 側の往復テストが見る**（`crates/app/tests/`）。盤面を保存して読み戻すテストはいまのまま残り、形が合っていることは `ts-rs` の生成物の差分が見る（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）
- **書き出す JSON の形は、置き場所をまたいで 1 つに固定する。** 同じファイル（`web/src/store/export.fixture.json`）を Rust と TypeScript の両方のテストが読み、どちらかがずれた日に両方が落ちる
- 殻そのもの（ネイティブのメニュー、OS の保存ダイアログ、グローバルホットキー、窓の矩形）は、[0021](0021-two-layer-testing-for-the-webview.md) のまま**手で確かめる**。ここは変えない
- webview のエンジンの差は [0023](0023-verifying-the-webview-engines.md) のまま、Chromium と WebKit の 2 つで回す

## 理由

**守るものが無くなった層だから。** [0021](0021-two-layer-testing-for-the-webview.md) の禁止事項は「モデルの挙動がテストの中でだけ違う」ことだった。モデルが 1 つしかなく、それが画面と同じ言語で同じ場所にあるなら、テストの中でだけ違うものを作る方法がそもそも無い。

**入口が 1 つになるから。** いま `make e2e` は `cargo run -p ekanban-harness` をテストごとに立てる。画面を 1 行直して確かめるのに Rust のビルドを待つ。移したあとは `npm` だけで回る。

**確かめる対象を、確かめられる場所に置くから。** 「TypeScript が書いた JSON を SQLite が受け取れるか」は Rust のテストで直接書ける。いまはそれが Playwright 経由の遠回りになっていて、失敗したときにどちらの層かを切り分ける手間がある。

## 採らなかった案

- **ハーネスを残し、TypeScript のモデルを Node で HTTP に出す。** 画面が直に呼べるものを、わざわざ HTTP に出して呼び直すことになる。何も守らない層が 1 つ残るだけ
- **e2e を Tauri の実物（`tauri-driver`）で回す。** WKWebView / WebView2 / WebKitGTK の実物を動かせるのは利点だが、3 OS ぶんの CI と、そこで出る不安定さを抱えることになる。[0023](0023-verifying-the-webview-engines.md) が「エンジンの系統で確かめ、実物はリリース前に手で見る」と決めた判断は、その費用を見たうえでのものである。ここは変えない
- **置き場所を、窓ごとの写しにする。** ページの中に閉じた入れ物なら `localStorage` を触らずに済むが、**2 つの窓が同じ盤面を見るところが試せなくなる**。キャプチャの窓が書いたことがボードの窓に届くのは、この機能の受け入れ条件そのものである。`localStorage` を差し、テストごとに蒔き直す形にした——`storage` の出来事がそのままブラウザ版の `board:changed` になる

## 結果

得るもの。画面のテストが `npm` だけで回り、Rust のビルドを待たない。テストごとにプロセスを立てて 250ms ごとに繋がるまで待つ、という起動の作りが消える。`crates/harness`（HTTP に出す 267 行）と `web/src/ipc/harness.ts` が消え、`web/e2e/harness.ts` はページに盤面を蒔くだけになる。**e2e が 4 分から 1 分半になった。**

引き受ける不都合。

- **「画面 → SQLite」の通し確認が無くなる。** 画面が書いた文書は e2e が見て、その文書を SQLite が受け取れるかは Rust のテストが見る。**間の 1 本のつなぎ目だけ、自動では確かめられない**。境界の型は `ts-rs` の生成物の差分が、書き出す JSON は共有の fixture が押さえるが、つなぎ目そのものはリリース前の手の確認が引き受ける
- **ブラウザの置き場所は本物の SQLite ではない。** 置き場所の口を満たしているだけで、書き込みの失敗（ディスクが一杯、読み取り専用、壊れている）をそのまま出すわけではない。失敗の出し分けは `error.rs` の Rust 側のテストが引き受ける
- **盤面の土台が 2 つになる。** マニュアルのスクリーンショット用（`crates/app/examples/manual_screenshot_seed.rs`、SQLite を作る）と、画面のテスト用（`web/e2e/fixture.ts`）。出力が違うので実装も分かれるが、**中身は揃えておく**——テストが見ている画面とマニュアルに載る画面が別物になると、説明のほうが嘘になる
