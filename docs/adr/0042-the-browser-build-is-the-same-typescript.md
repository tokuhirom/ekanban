# 0042. ブラウザ版を、アプリと同じ TypeScript で作る

- 日付: 2026-09-08
- 状態: 草案（未確定）
- 関連: [0035](0035-a-browser-build-of-the-real-core.md)（これを置き換える）、[0036](0036-one-model-two-places-to-put-it.md)、[0039](0039-the-board-model-moves-to-typescript.md)

## 状況

[0035](0035-a-browser-build-of-the-real-core.md) は「ブラウザで試せる版は、**本物の中核を wasm に組み直して**作る」と決めた。理由は [0021](0021-two-layer-testing-for-the-webview.md) と同じで、デモの中でだけ正しいものを作らないためである。そのために `crates/web`（351 行）があり、`wasm-pack` と `wasm32-unknown-unknown` のターゲットがビルドの前提にあり、SQLite を積めないので置き場所を 2 つに分けた（[0036](0036-one-model-two-places-to-put-it.md)、`store.rs` 687 行）。

[0039](0039-the-board-model-moves-to-typescript.md) で盤面のモデルが TypeScript に移ると、**ブラウザ版はアプリと同じコードがそのまま動く**。組み直すものが無い。

## 決定

**`crates/web` を畳む。** ブラウザ版は `web/demo/` が置き場所に `store/local`（`localStorage` の JSON）を差して起動するだけにする。

- 盤面のコードはアプリと 1 文字も違わない。違うのは差した置き場所と、環境の口の実装だけ
- ブラウザにできないこと（グローバルホットキー、ファイル管理で場所を開く、データベースのコピー）は、[0035](0035-a-browser-build-of-the-real-core.md) のまま**消さずに灰色にして、理由を文言に入れる**
- どの OS かをページが名乗る扱いも [0035](0035-a-browser-build-of-the-real-core.md) のまま（`web/demo/main.tsx` が `start()` に渡す）。この例外を `web/src/` に広げない
- `web/e2e-demo/` の役目も変えない——**ブラウザ版だけが持つ差**（再読み込みで盤面が残ること）を見る。盤面の振る舞いは `web/e2e/` が既に見ている

## 理由

**[0035](0035-a-browser-build-of-the-real-core.md) の目的が、より直接に満たされるから。** 「本物と同じものが動く」ために wasm に組み直していた。同じ TypeScript がそのまま動くなら、組み直す工程は目的ではなく費用だけになる。

**ビルドの前提が減るから。** `wasm-pack` と `wasm32-unknown-unknown` のターゲットが要らなくなる。`.github/workflows/pages.yml` は `npm run build` だけになる。**「`Check and test` は wasm を組まない」という、いま気をつけている落とし穴も消える**（`AGENTS.md`）。

**配るものが小さくなるから。** ページから wasm が消える。

**置き場所を 2 つに分ける理由も消えるから。** [0036](0036-one-model-two-places-to-put-it.md) が `Store` を作ったのは、ブラウザに SQLite を積めないからだった。TypeScript から見れば、置き場所は最初から差し替えられるものの 1 つでしかない。

## 採らなかった案

- **wasm を残し、置き場所だけを差し替える。** 同じ論理を 2 つの形（TypeScript と wasm）でビルドすることになる。ブラウザ版のためだけに、モデルを Rust に残す理由にもなる
- **ブラウザ版をやめる。** README が「インストールせずに試せます」を入口にしており、これは配布物の代わりを果たしている。やめる理由は無い
- **ブラウザ版に、アプリより機能を減らした版を置く。** [0035](0035-a-browser-build-of-the-real-core.md) の「消さずに灰色にする」を捨てることになる。何が違うのかを画面で読めなくする

## 結果

得るもの。ブラウザ版とアプリの差が、**置き場所と環境の口だけ**になる。`crates/web` と `store.rs` が消え、`wasm-pack` がビルドの前提から外れる。ページが軽くなる。

引き受ける不都合。

- **「本物の Rust が動いている」という説明が使えなくなる。** README と ADR の該当箇所を書き換える。代わりに言えるのは「アプリと同じコードが動いている」で、[0035](0035-a-browser-build-of-the-real-core.md) が本当に欲しかったのはこちらである
- **`localStorage` の上限（おおむね 5MB）に、盤面が文字列 1 つとして当たる。** [0036](0036-one-model-two-places-to-put-it.md) の JSON 置き場と同じ性質で、新しい制約ではない
