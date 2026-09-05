# 開発の手引き

ekanban に手を入れる人向けの文書です。

- 使う人向けの入口は [README](../README.md)、使い方は [マニュアル](MANUAL.md) にあります
- 「なぜそう作ってあるか」と、従うべき決まりごとは [設計の記録](DESIGN.md) にあります
- コーディング規約とテストの方針は [AGENTS.md](../AGENTS.md) にあります

---

## 構成

```text
src/
  main.rs         バイナリのエントリポイント
  lib.rs          データベースとウィンドウの初期化
  model.rs        Board / Column / Card などのドメインモデルと移動・並べ替え
  actions.rs      GPUI のアクション定義
  backup.rs       起動時の日ごと世代バックアップ（置き場所・命名・世代数）
  instance.rs     同じデータベースを 2 プロセスに開かせないロック
  menu.rs         ネイティブメニューバー、画面内メニューの項目、ショートカットの割り当て
  hotkey.rs       グローバルホットキーの登録と、環境ごとの利用可否の判定
  paths.rs        OS ごとのデータベースとログの配置の解決
  diagnostics.rs  起動失敗とパニックのログ記録、ダイアログ表示
  db/
    mod.rs        SQLite のスキーマ移行、読み書き、トランザクション
  views/
    mod.rs
    board.rs      ボードの描画、入力、ドラッグ＆ドロップ
    board/
      view_tests.rs  ウィンドウを開いて確かめるテスト
    capture.rs    クイックキャプチャの 1 行ウィンドウ
    window_chrome.rs  装飾を寄越さない環境で出す自前のタイトルバー
    description_links.rs  説明欄の中で URL をリンクとして見せ、開けるようにする
```

- **UI から SQL を直接実行しません。** SQL は `src/db/` に閉じます
- テストは実装と同じモジュールの `#[cfg(test)]` に置きます。データベースのテストは `tempfile` を使い、実物のデータベースを触りません。ビューのテストについては [テスト](#テスト) を見てください
- カード移動やカラム移動の保存は、必ず 1 つのトランザクションで行います

## UI の基盤

UI には [GPUI Kit](https://github.com/longbridge/gpui-kit) を使います。GPUI 本体と platform 層を個別に管理せず、GPUI Kit のテーマ・コンポーネント・入力処理に乗ります。

ドラッグ＆ドロップは GPUI の `on_drag` / `on_drop` を使い、見た目は GPUI Kit のコンポーネントとテーマに合わせます。色は `ActiveTheme::theme()` から引きます（`rgb(0x…)` の直書きは、ユーザーが指定したタグの色だけに許しています）。

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

スキーマは v10 です。移行は起動時に自動で走ります（`src/db/mod.rs` の `migrate`）。

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
- 入力は GPUI の入力コンポーネントに任せ、IME の composition を扱わせます
- 日本語の文字列をキーイベントから自前で組み立てません
- IME の変換中の Enter や Escape を、ショートカットとして取り上げません
- 入力欄を追加・変更したら、日本語 IME での入力を実機で確認してください

## テスト

テストは 2 種類あります。

**関数のテスト。** 実装と同じモジュールの `#[cfg(test)]` に置きます。モデルの並べ替え、期限の判定、エラー文言の組み立てのように、ウィンドウが無くても答えの出るものはこちらで書きます。データベースのテストは `tempfile` を使い、実物のデータベースを触りません。

**ビューのテスト。** `src/views/board/view_tests.rs` にあります。GPUI にはヘッドレスのテスト用プラットフォーム（`TestPlatform`）があり、`#[gpui_kit::test]` を付けると GPU もウィンドウマネージャも無いまま `App` と `Window` が立ち上がります。ここでは `BoardView` を本物のウィンドウに載せ、キー入力とアクションを流し込んで、画面の状態と SQLite に書かれた内容の両方を確かめます。使うには `test-support` feature が要るので、`Cargo.toml` の `[dev-dependencies]` で `gpui-kit` にだけ付けてあります（製品ビルドには入りません）。

```rust
#[gpui_kit::test]
fn adding_a_card_and_saving_it_writes_the_title_to_the_database(cx: &mut TestAppContext) {
    let (harness, cx) = open_board(cx);
    focus_board(&harness, cx);

    cx.dispatch_action(AddCard);
    cx.run_until_parked();
    cx.simulate_input("牛乳を買う");
    cx.dispatch_action(SaveEdit);
    cx.run_until_parked();

    assert!(harness.stored_board().columns[0]
        .cards
        .iter()
        .any(|card| card.title == "牛乳を買う"));
}
```

書くときの決まりごと:

- **待ち時間は `run_until_parked()` で決めます。** テストの時計は偽物なので、`sleep` は使いません。非同期の保存を挟む操作は、確かめる前に必ず `run_until_parked()` します
- **キー入力の前にボードへフォーカスを戻します**（`focus_board`）。入力欄にフォーカスがある間はボードのショートカットが効かないためです
- **割り当ては `crate::menu::install` が入れた本物を使います。** テスト用に定義し直すと、`src/menu.rs` の割り当てを変えたときにテストだけ通ってしまいます
- **確かめるのは画面とディスクの両方です。** `Harness::stored_board` がデータベースを開き直して読みます。メモリ上のモデルだけを見ると、保存の経路が壊れても気づけません
- ウィンドウのルートは本番と同じ `Root` にします。確認ダイアログと通知がここに載ります

`TestPlatform` は実際の描画も IME もウィンドウ管理もしません。日本語 IME での入力とライト／ダークの見え方は、これまで通り実機で確認してください。

## ビルド

`make help` でタスクの一覧が出ます。主なものは次の通りです。

| コマンド | 内容 |
| --- | --- |
| `make run` | ターミナルから直接起動する（デバッグビルド） |
| `make check` | CI と同じ fmt / clippy / test を走らせる |
| `make screenshots` | マニュアルのスクリーンショットを撮り直す（Linux/X11 のみ） |
| `make icon` | macOS 用の `assets/icon.icns` を `assets/icon.png` から生成する |
| `make bundle` | リリースビルドから `target/release/bundle/Ekanban.app` を作る |
| `make open` | `.app` を作って起動する |
| `make install` | `.app` を `/Applications` にコピーする |

`cargo build` が作るのは実行ファイルだけで、`.app` バンドルにはなりません。Dock のアイコンやアプリ名、Launchpad からの起動を正しく扱うには `make bundle` を使ってください。バンドル生成の実体は `script/bundle-mac` です。

`make bundle` は `assets/icon.png` から `assets/icon.icns` を生成してアイコンに取り込みます。生成には macOS の `sips` と `iconutil` が必要です。

### 署名

ローカルでは ad-hoc 署名（`-`）を使うので、追加の設定は要りません。配布用に Developer ID で署名する場合は環境変数で ID を渡します。

```sh
CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" make bundle
```

ad-hoc 以外の ID を指定したときは hardened runtime（`--options runtime`）とタイムスタンプが自動で付き、公証をそのまま通せる状態になります。entitlements が必要になったら `script/entitlements.plist` を置けば署名時に読み込まれます。

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
| `Check and test` | `ubuntu-latest` | `cargo fmt --all -- --check` / `cargo clippy --all-targets --all-features -- -D warnings` / `cargo test --all-features` / `cargo build --all-features` |
| `Build and test (macos-latest)` | `macos-latest` | `cargo test --all-features` / `cargo build --all-features` |
| `Build and test (windows-latest)` | `windows-latest` | `cargo test --all-features` / `cargo build --all-features` |

macOS と Windows を回すのは、そこでしかコンパイルされないコードがあるためです。`src/menu.rs` のネイティブメニューバー（`cx.set_menus` が実際に効くのは macOS だけ）、`src/paths.rs` と `src/diagnostics.rs` の `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` の分岐が該当します。fmt と clippy はプラットフォームに依らないので ubuntu だけで回します。

**`check` ジョブを matrix にしてはいけません。** matrix にすると check run の名前が `Check and test (ubuntu-latest)` になり、ルールセットが必須にしている `Check and test` がどこにも現れなくなって、すべての pull request がマージ不能になります。プラットフォームを足すときは、別ジョブとして足してください。

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
| macOS (Apple Silicon) | `macos-latest` | `ekanban-<版>-aarch64-apple-darwin.zip`（`Ekanban.app`） |
| Linux (x86_64) | `ubuntu-24.04` | `ekanban-<版>-x86_64-unknown-linux-gnu.tar.gz`（実行ファイル + README + LICENSE） |
| Windows (x86_64) | `windows-latest` | `ekanban-<版>-x86_64-pc-windows-msvc.zip`（`ekanban.exe` + README + LICENSE） |

あわせて `SHA256SUMS.txt` を置きます。

- **Intel Mac 向けは出していません。** 要るようになったら `macos-15-intel` のジョブを足すか、`lipo` で universal binary にします
- **Linux は `ubuntu-24.04` でビルドします。** glibc 2.39 に依存するので、それより古いディストリビューションでは動きません。実行にはこのほか Vulkan のドライバと fontconfig が要ります。`ubuntu-22.04` は 2026-09-17 から段階的に廃止されるので使いません
- **Windows のバイナリは、ビルドが通ることしか確かめていません。** クイックキャプチャは対象外のままです

### macOS の署名

いまは ad-hoc 署名のままです。ダウンロードした `.app` は Gatekeeper に止められるので、初回は右クリックから開く必要があります。

ワークフローは `CODESIGN_IDENTITY` シークレットがあれば `script/bundle-mac` にそのまま渡します。実際に Developer ID で署名するには、これに加えて次が要ります。

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

- **`versionFile = Cargo.toml`。** tag だけでなく `Cargo.toml` も上げます。`script/bundle-mac` がここから `.app` の `CFBundleShortVersionString` を作るので、置いていかれるとバンドルのバージョンがタグと食い違います
- **`postVersionCommand = cargo update --workspace`。** tagpr は `*.lock` を対象外にするので、これが無いと `Cargo.lock` の `ekanban` のバージョンだけ取り残されます

`main` のルールセットが `Check and test` を必須にしているので、tagpr の pull request も CI を通ってから merge されます。

tagpr は自分で GitHub Release を作ります。`release.yml` は、既にある Release には成果物を足すだけにしてあるので、上書きも二重作成も起きません。ただし Release が先にでき、ビルドが終わるまで数分は成果物が空になります。

ワークフローのアクションは [pinact](https://github.com/suzuki-shunsuke/pinact) でコミットハッシュに固定しています。バージョンを上げたら `pinact run` を掛け直してください。
