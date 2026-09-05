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
  menu.rs         ネイティブメニューバーとショートカットの割り当て
  hotkey.rs       グローバルホットキーの登録と、環境ごとの利用可否の判定
  paths.rs        OS ごとのデータベースとログの配置の解決
  diagnostics.rs  起動失敗とパニックのログ記録、ダイアログ表示
  db/
    mod.rs        SQLite のスキーマ移行、読み書き、トランザクション
  views/
    mod.rs
    board.rs      ボードの描画、入力、ドラッグ＆ドロップ
    capture.rs    クイックキャプチャの 1 行ウィンドウ
```

- **UI から SQL を直接実行しません。** SQL は `src/db/` に閉じます
- テストは実装と同じモジュールの `#[cfg(test)]` に置きます。データベースのテストは `tempfile` を使い、実物のデータベースを触りません
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

## ビルド

`make help` でタスクの一覧が出ます。主なものは次の通りです。

| コマンド | 内容 |
| --- | --- |
| `make run` | ターミナルから直接起動する（デバッグビルド） |
| `make check` | CI と同じ fmt / clippy / test を走らせる |
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

## 起動に失敗したときの記録

GUI から起動すると stderr がどこにも出ないため、起動時の致命的なエラーとパニックはログファイルに追記されます（置き場所は [README](../README.md#データの置き場所)）。あわせて、stderr が端末に繋がっていないとき（つまり GUI 起動のとき）だけダイアログでも知らせます。ターミナルから実行した場合はメッセージがそのまま見えるので、ダイアログは出ません。

ダイアログの表示には macOS では `osascript`、Windows では PowerShell、Linux/BSD では `zenity` / `kdialog` / `xmessage` のうち最初に見つかったものを使います。どれも無い環境ではログだけが残ります。

## CI

GitHub Actions（`.github/workflows/ci.yml`）が、`main` への push と pull request に対して次を実行します。

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo build --all-features`

`main` にはこのジョブ（`Check and test`）を必須にしたルールセットが掛かっているので、直接 push はできません。`main` からブランチを切り、`Closes #<issue>` を書いた pull request を出してください。
