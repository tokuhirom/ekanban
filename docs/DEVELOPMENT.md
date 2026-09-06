# 開発の手引き

ekanban に手を入れる人向けの文書です。

- 使う人向けの入口は [README](../README.md)、使い方は [マニュアル](MANUAL.md) にあります
- 「なぜそう作ってあるか」と、従うべき決まりごとは [設計の記録](DESIGN.md) にあります
- コーディング規約とテストの方針は [AGENTS.md](../AGENTS.md) にあります
- **UI は Tauri へ移す途中です。** いまの構造はこの文書のとおりですが、移行後に何がどうなるかは [Tauri 移行の設計](TAURI-MIGRATION.md) にあります（[ADR 0017](adr/0017-moving-the-ui-to-tauri.md)）

---

## 構成

Cargo のワークスペースです。**中核と UI を別のクレートに分けてあります**（[Tauri 移行の設計](TAURI-MIGRATION.md) §1）。

```text
crates/
  core/           ekanban-core: 盤面のモデル、SQLite、控え、置き場所
    src/
      lib.rs          アプリ名・識別子・データベースの置き場所
      model.rs        Board / Column / Card などのドメインモデルと移動・並べ替え
      backup.rs       起動時の日ごと世代バックアップ（置き場所・命名・世代数）
      instance.rs     同じデータベースを 2 プロセスに開かせないロック
      paths.rs        OS ごとのデータベースとログの配置の解決
      diagnostics.rs  起動失敗とパニックのログ記録、ダイアログ表示
      db/
        mod.rs        SQLite のスキーマ移行、読み書き、トランザクション
  harness/        ekanban-harness: コマンドを HTTP に出す。開発とテスト専用
  app/            ekanban-app: Tauri のアプリ（実行ファイルは ekanban-tauri）
    tauri.conf.json ウィンドウ、CSP、バンドルの設定
    capabilities/   webview に許すもの。使うものだけを並べる
    icons/          `tauri icon` が assets/icon.png から作ったもの
    src/
      run.rs          起動。多重起動の防止、控え、ウィンドウ
      menu.rs         メニューバーとキーの割り当て。まず「データ」として組む
      window.rs       ウィンドウの矩形を覚えて、次の起動で戻す
      capture.rs      クイックキャプチャの窓とグローバルな割り当て
      shortcut.rs     割り当ての形。保存も登録もここを通る
      ipc.rs          `#[tauri::command]` の包み。中身は持たない
      commands.rs     `docs/TAURI-MIGRATION.md` §3 のコマンド
      state.rs        開いている盤面。適用と保存をコマンドの中で終わらせる
      snapshot.rs     コマンドが返す形。起動時に読むもの
      error.rs        失敗の伝え方。入力欄に返すか、ダイアログに出すか
      events.rs       Rust から webview への 3 つのイベント
    tests/
      commands.rs     コマンドを外から呼んで、SQLite まで見るテスト
web/             画面。TypeScript + React + Vite（ADR 0019）
  src/
    ipc/          Rust を呼ぶ唯一の口。tauri と harness の 2 実装。
                  `types/` は ts-rs の生成物（手で書かない）
    state/        スナップショットの保持と、コマンドを呼んで差し替える 1 本の経路
    board/        サイドバー、ヘッダ、カラム、カード、D&D
      dnd.ts        どこに落ちるかの計算。**ライブラリの外に置く**（ADR 0022）
      keyboard.ts   矢印での選択と、修飾キー＋矢印での移動
    shell/        webview だから自分で切るもの（右クリック、拡大縮小、スワイプ）
    styles.css    色のトークンと骨組み
  e2e/            Playwright。ハーネス越しに Chromium と WebKit で動かす
harness/         ekanban-harness: コマンドを HTTP に出す開発・テスト専用のバイナリ
  examples/
    manual_screenshot_seed.rs  マニュアルのスクリーンショット用のデータベースを作る
```

- **`ekanban-core` に UI ツールキットを足しません。** gpui にも tauri にも依存しないことが、テストを GUI のランタイム無しで走らせ続ける条件であり、Tauri のアプリと開発用のハーネスが同じコードを使える条件でもあります（[Tauri 移行の設計](TAURI-MIGRATION.md) §1）。依存の依存から入り込むほうがありがちなので、解決した依存グラフを `script/check-core-independence` が CI で見ています
- **`crates/app/src/commands.rs` に `tauri` は出てきません。** `ipc.rs` の `#[tauri::command]` は、その関数を呼ぶだけの包みです。開発用のハーネス（[Tauri 移行の設計](TAURI-MIGRATION.md) §10）が同じ関数を HTTP に出すので、**判断を包みの側に置かないことは設計そのもの**です
- **D&D の挿入位置と、キーボードの割り当ては `web/src/board/dnd.ts` と `keyboard.ts` に置きます。** dnd-kit に渡すのは掴む・運ぶ・オートスクロールだけです（[ADR 0022](adr/0022-dnd-kit-core-for-drag-and-drop.md)）。盤面の意味を決めるところをライブラリに預けると、外せなくなります
- **どの OS で動いているかを `navigator.userAgent` から決めません。** あれは webview が書き換えられる文字列です（Playwright の Safari 模擬は Linux 上で `Macintosh` を名乗ります）。`secondary` が Cmd か Ctrl かを取り違えると割り当てが丸ごと効かないので、Rust が `StartupState.platform` で渡します（[ADR 0009](adr/0009-per-platform-key-bindings.md)、[ADR 0023](adr/0023-verifying-the-webview-engines.md)）
- **`crates/app` のコンパイルには `web/dist` が要ります。** `tauri::generate_context!` が画面を実行ファイルに埋め込むためです。checkout したてなら `npm --prefix web ci && npm --prefix web run build` を先に走らせてください（`make dev` と CI はそうしています）
- **Tauri のアプリは `make dev` で起動します。** デバッグビルドには Vite の開発サーバの URL が焼き込まれているので、`cargo run -p ekanban-app` だけでは白い画面になります
- **UI から SQL を直接実行しません。** SQL は `crates/core/src/db/` に閉じます
- **`web/src/ipc/types/` は手で書きません。** `ts-rs` が Rust の型から書き出します（`cargo test -p ekanban-core`、`make types`）。同じ型を 2 か所に書くと必ずずれるので、生成物をコミットして CI で差分を見ています（[Tauri 移行の設計](TAURI-MIGRATION.md) §3）。境界を越える値の決まり——**ID も時刻も JSON の数値**（`i64` を `bigint` にしない。`.cargo/config.toml` の `TS_RS_LARGE_INT`）、**期限は `"YYYY-MM-DD"` の文字列**、**時刻はエポックからのミリ秒**、鍵は camelCase——は `crates/core` のテストが見ています
- テストは実装と同じモジュールの `#[cfg(test)]` に置きます。データベースのテストは `tempfile` を使い、実物のデータベースを触りません。ビューのテストについては [テスト](#テスト) を見てください
- カード移動やカラム移動の保存は、必ず 1 つのトランザクションで行います

## UI の基盤

画面は Tauri の webview で、TypeScript + React + Vite で書きます（[ADR 0019](adr/0019-typescript-react-vite-for-the-webview.md)）。**盤面を持つのは Rust**で、webview が描くのはその投影です（[ADR 0018](adr/0018-rust-owns-the-board-state.md)）。

ドラッグ＆ドロップは `@dnd-kit/core` に載せますが、**どこに落ちるかを決めるのは `web/src/board/dnd.ts`** です（[ADR 0022](adr/0022-dnd-kit-core-for-drag-and-drop.md)）。色は `web/src/styles.css` のカスタムプロパティから引きます（直書きは、ユーザーが指定したタグの色だけに許しています）。

## ドラッグ＆ドロップの保存

ドラッグ中の状態はメモリ上だけで持ち、SQLite には書きません。

```text
ドラッグ開始
    ↓
移動先カラムと挿入位置を画面上に表示
    ↓
ドロップ
    ↓
カードの所属カラムと順番を更新
    ↓
SQLite に 1 トランザクションで保存
```

保存に失敗したら、ドラッグ前の状態に戻してエラーを出します。マウス移動のたびにデータベースへ触らないことで、操作中の応答性を保っています。

## データモデル

スキーマは v10 です。移行は起動時に自動で走ります（`crates/core/src/db/mod.rs` の `migrate`）。

```text
schema_migrations
  version
  applied_at

boards
  id
  name
  created_at
  updated_at
  next_card_id
  next_column_id
  next_tag_id
  next_checklist_item_id

columns
  id
  board_id
  name
  position
  created_at
  updated_at
  wip_limit (整数または NULL)

cards
  id
  column_id
  title
  description
  position
  created_at
  updated_at
  due_date (YYYY-MM-DD または NULL)
  archived_at (UNIX milliseconds または NULL)

tags
  id
  board_id
  name
  color
  created_at
  updated_at

card_tags
  card_id
  tag_id

checklist_items
  id
  card_id
  text
  checked
  position
  created_at
  updated_at

card_events
  id
  board_id
  card_id
  kind (created / moved / archived / restored / deleted)
  from_column_id
  to_column_id
  at

app_state
  key
  value
```

カードとカラムの順番は `position` で持ち、移動や並べ替えの完了時に対象範囲を振り直します。ローカル専用アプリなので、同期用の ID や競合解決は導入しません。

`app_state` に入るのは、ウィンドウの矩形、フィルターの状態、最後に開いたボード、テーマ設定、クイックキャプチャの割り当てと入れ先です。

**スキーマを変えたときは、旧バージョンのデータベースを開くマイグレーションテストを足してください。**

## 日本語の扱い

- SQLite には UTF-8 の文字列をそのまま保存します
- 入力は webview の入力欄に任せ、IME の composition を扱わせます
- 日本語の文字列をキーイベントから自前で組み立てません
- IME の変換中の Enter や Escape を、ショートカットとして取り上げません
- 入力欄を追加・変更したら、日本語 IME での入力を実機で確認してください

## テスト

テストは 4 つの層に分かれます（[ADR 0021](adr/0021-two-layer-testing-for-the-webview.md)）。

| 層 | 何で | 何を担保するか |
| --- | --- | --- |
| 中核 | `cargo test` | モデル・SQLite・移行・控え。実装と同じモジュールの `#[cfg(test)]` に置き、データベースのテストは `tempfile` を使う |
| コマンド | `crates/app/tests/commands.rs` | コマンドを外から呼び、**返るスナップショットと SQLite の中身の両方**を見る |
| 画面 | Playwright ＋ `ekanban-harness` | 操作からデータベースまでを通した振る舞い |
| 部品 | Vitest | 日付の表示、挿入位置の計算、キーの振り分けのような純粋な部分 |

**画面のテストはハーネス越しに動かします。** `crates/harness` が `crates/app` のコマンドをそのまま HTTP に出すので、同じ画面がふつうのブラウザで動きます。**通っているのは本物の `ekanban-core`** です——偽物のバックエンドを TypeScript で書くと、テストの中でだけ正しいものができあがります（[ADR 0021](adr/0021-two-layer-testing-for-the-webview.md)）。

```ts
test("カードを足して保存すると、タイトルがデータベースに入る", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("牛乳を買う");
  await page.locator(".save-card").click();

  // 画面ではなく、保存されたほうを読み直す。
  await expect.poll(storedTitles).toContain("牛乳を買う");
});
```

書くときの決まりごと:

- **確かめるのは画面とディスクの両方です。** `e2e/harness.ts` の `invoke()` がハーネスを直に叩いて盤面を読み直します。画面に出ているだけでは、保存の配線が抜けていても気づけません
- **盤面はテストごとに作り直します。** 1 つのデータベースを使い回すと、前のテストが動かしたカードの位置に次のテストが引きずられます
- **`sleep` で待ちません。** `expect.poll` と `toBeVisible` の待ちを使います
- **走らせるのは Chromium と WebKit の 2 つ**です。本物の webview は 3 つですが、エンジンは 2 系統しかありません（[ADR 0023](adr/0023-verifying-the-webview-engines.md)）

**ここに出てこないもの。** 本物のメニューバー、OS の保存ダイアログ、グローバルホットキー、ウィンドウの矩形——**Tauri の殻はブラウザには無い**ので、そこは実機で触って確かめます（[アプリを動かして確かめるとき](#アプリを動かして確かめるとき)）。日本語 IME での入力とライト／ダークの見え方も同じです。

## ビルド

`make help` でタスクの一覧が出ます。主なものは次の通りです。

| コマンド | 内容 |
| --- | --- |
| `make dev` | アプリを開発モードで起動する（Vite の開発サーバごと） |
| `make check` | CI と同じ fmt / clippy / test / 型 / 依存 / 画面側の確認を走らせる |
| `make types` | Rust の型から TypeScript の型を書き出す |
| `make web-check` | 画面側の `tsc --noEmit` / ESLint / Vitest |
| `make e2e` | ハーネス越しに Chromium と WebKit で画面を動かす |
| `make screenshots` | マニュアルのスクリーンショットを撮り直す（Linux/X11 のみ） |
| `make icon` | 3 つの OS 分のアイコンを `assets/icon.png` から生成する（`tauri icon`） |
| `make bundle` | この OS の配布物を作る（macOS は `.app` と `.dmg`、Linux は `.deb` と `.AppImage`、Windows はインストーラ） |
| `make open` | `.app` を作って起動する（macOS） |
| `make install` | `.app` を `/Applications` にコピーする（macOS） |
| `make install-linux` | Linux のアプリ一覧に登録する（`~/.local` 以下） |
| `make uninstall-linux` | `install-linux` で入れたものを消す |

`cargo build` が作るのは実行ファイルだけです。Dock のアイコンやアプリ名、Launchpad からの起動、アプリ一覧への登録を正しく扱うには `make bundle` を使ってください。組むのは Tauri のバンドラで、CLI は画面側の devDependencies に入っているので別に入れるものはありません。

アイコンは `crates/app/icons/` に生成したものをコミットしてあります（`.icns`・`.ico`・PNG 各種）。`assets/icon.png` を差し替えたときは `make icon` で作り直してください。**macOS のツール（`sips` / `iconutil`）は要りません**——`tauri icon` がどの OS でも 3 つ分を作ります。

### Linux のデスクトップ統合

Linux でも、実行ファイルだけではアプリ一覧に出ず、タスクバーのアイコンと名前も汎用のものになります。デスクトップ環境がウィンドウをアプリに結びつけるのはデスクトップエントリなので、それを入れる必要があります。

| ファイル | 置き場所 | 何のため |
| --- | --- | --- |
| `assets/dev.tokuhirom.ekanban.desktop` | `$XDG_DATA_HOME/applications` | アプリ一覧に出す。`StartupWMClass` がウィンドウとエントリを結びつける |
| `assets/icons/hicolor/<大きさ>/apps/dev.tokuhirom.ekanban.png` | `$XDG_DATA_HOME/icons/hicolor/…` | アイコン。`assets/icon.png` から縮小したものを 7 種類置いてある |
| 実行ファイル | `$XDG_BIN_HOME`（既定 `~/.local/bin`） | 本体 |

入れるのは `script/install-linux`（`make install-linux`）です。root は要りません。`--uninstall` で消します。エントリの `Exec=` は、入れた実行ファイルの絶対パスに書き換えてから置きます。`~/.local/bin` が PATH に入っていない環境でも一覧から起動できるようにするためです。

**`StartupWMClass` は、ウィンドウが名乗る `WM_CLASS` と必ず同じにしてください**（`crates/core/src/lib.rs` の `WM_CLASS`）。食い違うと、起動したウィンドウがそのエントリに結びつかず、タスクバーのアイコンと名前が元に戻ります。

**これは `APP_ID` ではありません。** Tauri（tao）は `WM_CLASS` を**実行ファイルの名前**から作るので、`ekanban` という実行ファイルは `("ekanban", "Ekanban")` と名乗ります。実機で確かめるなら `xprop WM_CLASS` です。`.deb` と `.AppImage` に入るエントリはバンドラが作るので、そちらを直したいときは `tauri.conf.json` を見てください。

アイコンは大きさごとに `assets/icons/` へコミットしてあります。ビルド時に縮小しないのは、Linux のランナーに画像処理のツールを増やさないためです。`assets/icon.png` を差し替えたときは、同じ 7 種類（16 / 32 / 48 / 64 / 128 / 256 / 512）を作り直してください。

### 署名

ad-hoc 署名（`-`）です。指定は `crates/app/tauri.conf.json` の `bundle.macOS.signingIdentity` にあり、ローカルでも CI でも同じものが使われます（[ADR 0014](adr/0014-unsigned-apple-silicon-only-macos-builds.md)）。

配布用に Developer ID で署名するなら、環境変数で ID を渡します。Tauri のバンドラは、これがあるときだけ hardened runtime とタイムスタンプを付けます。

```sh
APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" make bundle
```

## アプリを動かして確かめるとき

変更が画面でどう見えるかを確かめるときは、**仮想ディスプレイの上で動かします。**

```sh
# デバッグの実行ファイルには Vite の開発サーバの URL が焼き込まれる。
# `--debug` のバンドルは画面を埋め込むので、開発サーバなしで動かせる。
(cd crates/app && ../../web/node_modules/.bin/tauri build --debug --no-bundle)

Xvfb :99 -screen 0 1600x1200x24 &
DISPLAY=:99 EKANBAN_DATABASE=$(mktemp -d)/board.sqlite3 ./target/debug/ekanban &
DISPLAY=:99 import -window root shot.png
```

**ここでしか確かめられないものがあります**——OS のメニューバー、保存ダイアログ、グローバルホットキー、キャプチャの窓。ハーネス越しの Playwright にはどれも出てきません。

デスクトップで動いているものに紛れ込ませないためです。データベースも普段使いのものとは分けます。1 つのデータベースを開けるのは 1 プロセスだけ（[ADR 0004](adr/0004-one-process-per-database.md)）なので、同じものを指すと後から起動したほうが弾かれます。

キーやクリックを送るときは、次の 2 つを守ってください。

- **触るウィンドウを PID で照合する。** `xdotool search --name ekanban` は、すでに開いている別のインスタンスも一緒に拾います。`xdotool getwindowpid <id>` が自分で起動したプロセスと一致することを確かめてから送ります。確かめずに送って、別のインスタンスで編集中だったカードを取り消してしまったことがあります
- **`import -window root` で撮る。** メニューやポップアップは `deferred` で別の層に描かれるので、`import -window <id>` では写りません。「メニューが開いていない」と見えて、実際には開いていたことがあります

## マニュアルのスクリーンショット

[マニュアル](MANUAL.md) の画像は `docs/images/` に置き、`script/manual-screenshots` で撮り直します（`make screenshots` でも同じです）。

```sh
script/manual-screenshots              # 6 枚すべて
script/manual-screenshots board-dark   # 1 枚だけ
```

| ファイル | 見せているもの |
| --- | --- |
| `board.png` | ボードの全体 |
| `card-edit.png` | カードの編集パネル |
| `search.png` | 検索での絞り込み |
| `filter-tag.png` | タグでの絞り込み |
| `board-list-collapsed.png` | ボード一覧を畳んだところ |
| `board-dark.png` | ダークモード |

要るものは `Xvfb`、`xdotool`、ImageMagick の `import`、日本語フォント（`fonts-noto-cjk` など）です。`optipng` があれば自動で通し、無ければそのまま置きます。macOS では動きません。Linux（X11）で揃えているのは、誰でも同じものを撮り直せるようにするためです。マニュアルの冒頭にも、そう撮ったものだと断ってあります。

### 撮り方

盤面は `examples/manual_screenshot_seed.rs` が `EKANBAN_DATABASE` のデータベースを作り直して用意します。SQL を直接書かず、アプリ自身の API（`Database` と `Board`）で組み立てます。**撮れた絵が、アプリの本当に復元できる状態であること**を、作り方の側で保証するためです。

検索語、絞り込みのタグ、ボード一覧の開閉、テーマは `app_state` に残るので、seed が状態まで作ってからアプリを起動すれば、そのまま撮れます。カードの編集パネルだけは保存されないので、これは `script/manual-screenshots` がカードを実際に押して開きます。

ウィンドウマネージャは動かしません。飾り枠が付かないので、アプリの既定のウィンドウがそのまま 1200x800 で撮れます。撮る直前にポインタをカードの無いところへ逃がすのは、たまたま下にあったカードが hover の色で写り込まないようにするためです。

### 撮り直すときに気をつけること

- **期限は撮った日からの相対で入ります。** `期限切れ 2日 (9/3)` のような日付は撮る日によって変わります。マニュアルの「期限の書き分け」の表は画像と同じ日付を載せているので、撮り直したら表も直してください
- 画面を足すときは、`examples/manual_screenshot_seed.rs` の `SCREENS` と `script/manual-screenshots` の両方に名前を足します。その名前がそのまま `docs/images/<名前>.png` になります
- フォントが変わると折り返しも変わります。カードの説明が 2 行になって句点だけが取り残されるようなら、画像ではなく文言のほうを詰めてください

## 起動に失敗したときの記録

GUI から起動すると stderr がどこにも出ないため、起動時の致命的なエラーとパニックはログファイルに追記されます（置き場所は [README](../README.md#データの置き場所)）。あわせて、stderr が端末に繋がっていないとき（つまり GUI 起動のとき）だけダイアログでも知らせます。ターミナルから実行した場合はメッセージがそのまま見えるので、ダイアログは出ません。

ダイアログの表示には macOS では `osascript`、Windows では PowerShell、Linux/BSD では `zenity` / `kdialog` / `xmessage` のうち最初に見つかったものを使います。どれも無い環境ではログだけが残ります。

## CI

GitHub Actions（`.github/workflows/ci.yml`）が、`main` への push と pull request に対して次を実行します。

| ジョブ | ランナー | 実行するもの |
| --- | --- | --- |
| `Check and test` | `ubuntu-latest` | 画面側（`npm ci` / `vite build` / `tsc` / ESLint / Vitest）／`cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --all-features -- -D warnings` / `cargo test --workspace --all-features` / 生成した型の差分 / `cargo build --workspace --all-features` / `script/check-core-independence` |
| `Build and test (macos-latest)` | `macos-latest` | `npm ci` / `vite build` / `cargo test --workspace --all-features` / `cargo build --workspace --all-features` |
| `Build and test (windows-latest)` | `windows-latest` | `npm ci` / `vite build` / `cargo test --workspace --all-features` / `cargo build --workspace --all-features` |

3 つの OS すべてで画面を先に組み立てるのは、`crates/app` のコンパイルが `web/dist` を実行ファイルに埋め込むからです。型検査と lint はプラットフォームに依らないので ubuntu だけで回します。

macOS と Windows を回すのは、そこでしかコンパイルされないコードがあるためです。`crates/app/src/menu.rs` の OS ごとのメニューバー、`crates/core/src/paths.rs` と `crates/core/src/diagnostics.rs` の `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` の分岐が該当します。fmt と clippy はプラットフォームに依らないので ubuntu だけで回します。

**`check` ジョブを matrix にしてはいけません。**（この判断の経緯は [ADR 0006](adr/0006-ci-on-three-platforms.md)） matrix にすると check run の名前が `Check and test (ubuntu-latest)` になり、ルールセットが必須にしている `Check and test` がどこにも現れなくなって、すべての pull request がマージ不能になります。プラットフォームを足すときは、別ジョブとして足してください。

`main` には `Check and test` を必須にしたルールセットが掛かっているので、直接 push はできません。`main` からブランチを切り、`Closes #<issue>` を書いた pull request を出してください。

## リリース

タグを打つと `.github/workflows/release.yml` がビルドして、GitHub Release に成果物を上げます。

```sh
git switch main && git pull
git tag v0.1.0
git push origin v0.1.0
```

タグは `v` から始めます。そうでないものはワークフローが弾きます。

### 出るもの

| プラットフォーム | ランナー | 成果物 |
| --- | --- | --- |
| macOS (Apple Silicon) | `macos-latest` | `ekanban-<版>-aarch64-apple-darwin.zip`（`Ekanban.app`）と `.dmg` |
| Linux (x86_64) | `ubuntu-24.04` | `ekanban-<版>-x86_64-unknown-linux-gnu.tar.gz`（実行ファイル + README + LICENSE + `dev.tokuhirom.ekanban.desktop` + `icons/` + `install-linux`）、`.deb`、`.AppImage` |
| Windows (x86_64) | `windows-latest` | `ekanban-<版>-x86_64-pc-windows-msvc.zip`（`ekanban.exe` + README + LICENSE）と `-setup.exe`（NSIS） |

**パッケージと一緒に、素の実行ファイルも配り続けます。** `.deb` を入れるには root が要り、それを求めない導線を残すというのが [ADR 0013](adr/0013-linux-desktop-integration.md) の決定だからです。

あわせて `SHA256SUMS.txt` を置きます。

- **Intel Mac 向けは出していません。** 判断と理由は [`docs/DESIGN.md`](DESIGN.md) と [ADR 0014](adr/0014-unsigned-apple-silicon-only-macos-builds.md) にあります。出すことにしたら、`macos-15-intel` のジョブを足すか、`lipo` で universal binary にします
- **Linux は `ubuntu-24.04` でビルドします。** glibc 2.39 に依存するので、それより古いディストリビューションでは動きません。実行には WebKitGTK 4.1 と GTK 3 が要ります。`ubuntu-22.04` は 2026-09-17 から段階的に廃止されるので使いません
- **xdg-desktop-portal は要りません**（[ADR 0024](adr/0024-no-portal-requirement-on-linux.md)）。保存ダイアログは GTK の口、テーマの「システムに合わせる」は CSS の `prefers-color-scheme` です。ポータルに触れるのは「場所を開く」だけで、そこもファイル管理（`org.freedesktop.FileManager1`）→ ポータル → `xdg-open` の順に試します
- **Windows のバイナリは、ビルドが通ることしか確かめていません。** クイックキャプチャは対象外のままです

### macOS の署名と公証

いまは ad-hoc 署名のままです。ダウンロードした `.app` は Gatekeeper に止められるので、初回だけ手順が要ります。**その手順は macOS 15 (Sequoia) で変わりました。**

| macOS | 初回の開き方 |
| --- | --- |
| 15 (Sequoia) 以降 | 一度ダブルクリックして弾かれたあと、**システム設定 > プライバシーとセキュリティ** の「このまま開く」を押す。Control クリックからの回避は塞がれた |
| 14 (Sonoma) 以前 | **Control クリック > 開く** |

手順は README に書いてあります。**片方だけ直さないこと。** バージョンによって通らない案内は、通らない側の人にとっては「壊れている」のと同じです。

Developer ID での署名と公証をやらない判断と、その理由は [`docs/DESIGN.md`](DESIGN.md) と [ADR 0014](adr/0014-unsigned-apple-silicon-only-macos-builds.md) にあります。やるときに要るものは次の通りです。

実際に Developer ID で署名するには、次が要ります（いまのワークフローは ad-hoc のまま組みます）。

- Apple Developer Program の登録（年額）
- 証明書（`.p12`）をシークレットに入れて、ビルド前に一時キーチェーンへ取り込むステップ
- `xcrun notarytool submit --wait` と `xcrun stapler staple` による公証

### ビルドだけ確かめる

Actions の Release ワークフローを `workflow_dispatch` で、`tag` を空のまま実行すると、3 つのプラットフォームでビルドが通るかだけ見ます。リリースには何も上がりません。

### tagpr

バージョン上げとタグ打ちは [tagpr](https://github.com/Songmu/tagpr) に任せています（`.github/workflows/tagpr.yml`）。

```text
main に push
    ↓
tagpr がリリース用の pull request を作る / 更新する
    ↓
その pull request を merge
    ↓
tagpr が Cargo.toml と Cargo.lock を上げ、タグを打ち、GitHub Release を作る
    ↓
タグの push が release.yml を動かし、成果物を Release に足す
```

バージョンは semver です。上げ幅は merge した pull request のラベルで決まります。`major` が付いていれば major、`minor` なら minor、どちらも無ければ patch です。

**タグを打つのは GitHub App のインストールトークンです。** `GITHUB_TOKEN` で作られたイベントは他のワークフローを起動しないため、それだとタグを打っても `release.yml` が動きません。App のトークンにはその制限が掛からないので、タグがそのままリリースまで繋がります。次の 2 つがリポジトリに要ります。

| 種類 | 名前 | 中身 |
| --- | --- | --- |
| Variable | `TAGPR_APP_ID` | GitHub App の App ID |
| Secret | `TAGPR_APP_PRIVATE_KEY` | その App の秘密鍵 |

App には Contents と Pull requests の write 権限が要ります。あわせて Settings → Actions → General の「Allow GitHub Actions to create and approve pull requests」を有効にしてください。無効のままだと tagpr がリリース用の pull request を作れません。

`.tagpr` で気をつけているところが 2 つあります。

- **`versionFile = Cargo.toml`。** tag だけでなくワークスペースのルートの `Cargo.toml`（`[workspace.package]` の `version`）も上げます。Tauri のバンドラがここから `.app` の `CFBundleShortVersionString` や `.deb` のバージョンを作るので、置いていかれるとパッケージのバージョンがタグと食い違います。行頭の `version = "..."` はこの 1 か所だけにしてください。両方とも先頭の 1 つを見ています
- **`postVersionCommand = cargo update --workspace`。** tagpr は `*.lock` を対象外にするので、これが無いと `Cargo.lock` の `ekanban` と `ekanban-core` のバージョンだけ取り残されます

`main` のルールセットが `Check and test` を必須にしているので、tagpr の pull request も CI を通ってから merge されます。

tagpr は自分で GitHub Release を作ります。`release.yml` は、既にある Release には成果物を足すだけにしてあるので、上書きも二重作成も起きません。ただし Release が先にでき、ビルドが終わるまで数分は成果物が空になります。

ワークフローのアクションは [pinact](https://github.com/suzuki-shunsuke/pinact) でコミットハッシュに固定しています。バージョンを上げたら `pinact run` を掛け直してください。
