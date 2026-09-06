# 0023. webview の差は、エンジンの系統で確かめる

- 日付: 2026-09-06
- 状態: 有効
- 関連: [0020](0020-pointer-based-drag-and-drop.md)、[0021](0021-two-layer-testing-for-the-webview.md)、[0022](0022-dnd-kit-core-for-drag-and-drop.md)、[0009](0009-per-platform-key-bindings.md)

## 状況

[0020](0020-pointer-based-drag-and-drop.md) の受け入れ条件 8 は「3 つの webview（WKWebView / WebView2 / WebKitGTK）で 1〜7 が同じ」である。[0022](0022-dnd-kit-core-for-drag-and-drop.md) はこれを「macOS と Windows の実機が要るため、手での確認に残る」とした。

**それは早かった。** 3 つの webview が使っているエンジンは 2 系統しかない。

| webview | エンジン |
| --- | --- |
| WebKitGTK（Linux） | WebKit |
| WKWebView（macOS） | WebKit ＋ Apple の platform 層 |
| WebView2（Windows） | Chromium／Edge |

そして [0021](0021-two-layer-testing-for-the-webview.md) が「開発とテスト専用に `crates/core` を HTTP に出す」と決めたハーネスがあれば、同じ画面をふつうのブラウザで動かせる。Playwright が繋がるのは Chromium と WebKit——**まさにその 2 系統**である。

## 決定

**条件 8 は、エンジンの系統ごとに CI で確かめる。** ハーネス越しに同じ画面を Chromium と WebKit で動かし、条件 1〜6 を毎回走らせる。**残すのは Apple の platform 層の確認だけ**——慣性スクロール、ゴムのような跳ね返り、trackpad の 2 本指——で、そこは macOS の実機で人が触る。

**そのために `crates/harness` を段階 5 より前に作る。** [0021](0021-two-layer-testing-for-the-webview.md) は §10 の道具として書いたが、条件 8 を確かめるのにこちらが先に要る。

**動いている OS を `navigator.userAgent` から決めない。** Rust がコンパイル時に知っていることを `StartupState.platform` で渡す。

## 理由

**「実機が無い」は、確かめない理由になっていなかったから。** 手元に無いのは macOS と Windows という**機械**であって、Chromium と WebKit という**エンジン**ではない。条件 8 が本当に問うているのは後者である。系統の差を毎回見て、機械の差だけを手に残すほうが、両方まとめて手に残すより確実に多くを捕まえる。

**実際、跨いで走らせた最初の回に 1 つ出たから。** キーの割り当てを `navigator.userAgent` から決めていたところ、Playwright の Safari 模擬が Linux 上で `Macintosh` を名乗り、`secondary` が Cmd と読まれて `Ctrl+Alt` の割り当てが丸ごと死んだ。WebKitGTK だけを見ていれば通っていた。

**UA を信じないのは、あれが webview の書き換えられる文字列だから。** [0009](0009-per-platform-key-bindings.md) が決めた OS ごとの割り当ては、`secondary` が Cmd か Ctrl かを取り違えると**丸ごと効かなくなる**。効かないことは静かなので、間違えても誰も気づかない。Rust の `cfg!` は嘘をつかない。

**ハーネスを先に作るのが安いから。** 段階 2 でコマンド層を `tauri` から切り離してあるので（`crates/app/src/commands.rs`）、HTTP に出すのは振り分けを書くだけで済む。通るのは本物の `ekanban-core` なので、[0021](0021-two-layer-testing-for-the-webview.md) が禁じた「偽物のバックエンドを TypeScript で書く」ことにはならない。

## 採らなかった案

- **実機だけで確かめる（[0022](0022-dnd-kit-core-for-drag-and-drop.md) の当初）。** エンジンの差に気づくのがリリース前だけになる。上の不具合はそこまで残っていた
- **`tauri-driver` で本物の webview を叩く。** macOS に無い（[0021](0021-two-layer-testing-for-the-webview.md)）。3 つ揃わないなら、揃う 2 系統を毎回見るほうがよい
- **Playwright のブラウザを本物の webview の代わりだと言う。** 言わない。WKWebView は Apple の platform 層を持ち、WebView2 は埋め込み方が違う。**ここで担保できるのはエンジンの系統までだと、はっきり書いておく**

## 結果

得るもの。エンジンの系統ごとの差が CI で毎回見える。webview だけを差し替えた検証ができるので、原因の切り分けが速い。ハーネスは段階 5 の Playwright にそのまま使える。

引き受ける不都合。

- **Playwright のブラウザは本物の webview ではない。** ここが緑でも、WKWebView と WebView2 で同じとは言い切れない。**言い切らない**
- **Apple の platform 層は残る。** 慣性スクロール、跳ね返り、trackpad。macOS の実機で人が触るしかない
- **CI が長くなる。** ブラウザを 2 つ入れて 14 本走らせるぶん。条件 8 を毎回見る値段として払う
