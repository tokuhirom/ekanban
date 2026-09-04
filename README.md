# ekanban

Rust と [GPUI Kit](https://github.com/longbridge/gpui-kit) で作る、ローカル専用の Kanban アプリです。

カードをドラッグ＆ドロップして、直感的にカラム間を移動したり、カラム内の順番を変更したりできることを最も重視します。

## 方針

- データはローカルの SQLite に保存する
- クラウド同期は実装しない
- アカウント、サーバー、ネットワーク接続を必要としない
- 日本語を入力・表示できるようにする
- ドラッグ中は UI 上で滑らかにカードを移動し、ドロップ時にだけ SQLite を更新する

## MVP

### ボード

- カラムを横方向に表示する
- カラムの追加、名前変更、削除
- カラムのドラッグ＆ドロップによる並べ替え

### カード

- カードの追加、編集、削除
- カラム内での並べ替え
- 別カラムへの移動
- 空のカラムへのドロップ
- タイトルと説明の編集

カード内の編集・削除ボタンは、ドラッグ開始の対象から除外します。ドラッグ中はカードやカラムのゴーストと、ドロップ対象の強調表示を表示します。

期限、タグ、検索、Markdown、通知、複数ユーザー、クラウド同期は MVP の対象外です。期限、タグ、検索、アーカイブ、Undo/Redo、非同期保存は先行して実装済みで、複数ボード以降は引き続き拡張中です。詳しくは [実装ロードマップ](docs/ROADMAP.md) を参照してください。

## ドラッグ＆ドロップ

ドラッグ中の状態はメモリ上で管理し、SQLite には保存しません。

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

保存に失敗した場合は、ドラッグ前の状態に戻してエラーを表示します。マウス移動のたびにデータベースへアクセスしないことで、操作中の応答性を保ちます。

## データモデル

```text
boards
  id
  name
  created_at
  updated_at
  next_card_id
  next_column_id
  next_tag_id

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
```

データベースは自動マイグレーションに対応します。カードやカラムの順番は `position` で管理し、移動や並べ替えの完了時に対象範囲の順番を振り直します。ローカル専用アプリのため、複雑な同期用 ID や競合解決は導入しません。

## GPUI Kit

UI の基盤には [GPUI Kit](https://github.com/longbridge/gpui-kit) を使用します。GPUI 本体と対応する platform 層を個別に管理せず、GPUI Kit のテーマ・コンポーネント・入力処理を利用します。

D&D のカード操作は GPUI の `on_drag` / `on_drop` を使い、カードやカラムの見た目は GPUI Kit のコンポーネントとテーマに合わせます。

## 構成

```text
src/
  main.rs
  lib.rs
  model.rs
  paths.rs
  diagnostics.rs
  db/
    mod.rs
  views/
    mod.rs
    board.rs
```

- `model.rs`: Board、Column、Card などのドメインモデル
- `db/mod.rs`: SQLite の読み書きとスキーマのマイグレーション
- `views/`: GPUI による表示、入力、ドラッグ＆ドロップ
- `paths.rs`: OS ごとのデータベースとログの配置の解決
- `diagnostics.rs`: 起動失敗とパニックのログ記録、ダイアログ表示
- UI から SQL を直接実行しない

カード移動やカラム移動の保存は、必ず 1 つのトランザクションで行います。保存処理は今後、データ量が増えた場合に UI スレッドをブロックしない実行方式へ分離します。

## 日本語対応

- SQLite には UTF-8 の文字列をそのまま保存する
- GPUI の入力コンポーネントで IME composition を扱う
- 日本語文字列をキーイベントから自前で組み立てない
- IME 変換中の Enter や Escape を誤ってショートカット処理しない
- 日本語のタイトルと説明を保存・再表示できることを確認する

## 実装状況と次の実装

完了しているもの:

1. Rust/Cargo プロジェクト、GPUI Kit のテーマ、コンポーネントを初期化する
2. SQLite の自動マイグレーションとデモデータの投入
3. カラムとカードの表示
4. カードの追加
5. カードのドラッグ＆ドロップ（カラム間移動、カラム内並べ替え、空カラム）
6. カラムのドラッグ＆ドロップによる並べ替え
7. 移動後の SQLite 保存と保存失敗時のロールバック
8. カードの編集・削除と保存失敗時のロールバック
9. 単調増加 ID 採番と差分保存
10. カラムの追加・名前変更・削除、削除確認
11. 期限の保存・表示・編集、期限順並べ替え、期限フィルター
12. タイトル・説明の検索、WIP 上限の警告表示
13. タグの追加・編集・削除、カードへの付け外し、色付きチップ、タグフィルター
14. カード・カラムのアーカイブ、アーカイブ一覧、カードの復元
15. 操作単位の Undo/Redo（`Ctrl+Z` / `Ctrl+Shift+Z`）
16. 保存の非同期化（保存中の操作キュー、完了・失敗通知、失敗時のロールバック）

次に実装するもの:

1. 複数ボードとキーボード操作（フェーズ 4-3 / 4-4）
2. エラー表示の改善（フェーズ 4-5）
3. チェックリスト、カードのコピー、コンテキストメニュー（フェーズ 5）

フェーズごとの作業内容、設計判断、受け入れ条件は [実装ロードマップ](docs/ROADMAP.md) にまとめています。

最初のプロトタイプは、3 カラムと数枚のカードを表示し、カードとカラムをドラッグ＆ドロップしてローカル SQLite に保存できるところまで実装しています。

## 必要なもの

- Rust toolchain
- SQLite

## ビルドと起動

`make help` でタスク一覧が出ます。主なものは次の通りです。

| コマンド | 内容 |
| --- | --- |
| `make run` | ターミナルから直接起動する (デバッグビルド) |
| `make check` | CI と同じ fmt / clippy / test を走らせる |
| `make bundle` | リリースビルドから `target/release/bundle/Ekanban.app` を作る |
| `make open` | `.app` を作って起動する |
| `make install` | `.app` を `/Applications` にコピーする |

`cargo build` が作るのは実行ファイルだけで、`.app` バンドルにはなりません。Dock のアイコンやアプリ名、Launchpad からの起動を正しく扱うには `make bundle` を使ってください。バンドル生成の実体は `script/bundle-mac` です。

`assets/icon.icns` を置くと、アイコンとして自動的に取り込まれます。

### データベースの置き場所

OS ごとの標準の場所に保存します。GUI から起動するとカレントディレクトリが当てにならないため、相対パスは使いません。

| OS | データベース | ログ |
| --- | --- | --- |
| macOS | `~/Library/Application Support/ekanban/ekanban.sqlite3` | `~/Library/Logs/ekanban.log` |
| Linux/BSD | `$XDG_DATA_HOME/ekanban/` または `~/.local/share/ekanban/` | `$XDG_STATE_HOME/ekanban/` または `~/.local/state/ekanban/` |
| Windows | `%APPDATA%\ekanban\` | `%LOCALAPPDATA%\ekanban\` |

別の場所を使いたい場合は `EKANBAN_DATABASE` で上書きできます。

```sh
EKANBAN_DATABASE=./dev.sqlite3 make run
```

### 起動に失敗したとき

GUI から起動すると stderr がどこにも表示されないため、起動時の致命的なエラーとパニックは上表のログファイルに追記されます。あわせて、stderr が端末に繋がっていないとき (つまり GUI 起動のとき) だけダイアログでも通知します。ターミナルから実行した場合はメッセージがそのまま見えるので、ダイアログは出ません。

ダイアログの表示には macOS では `osascript`、Windows では PowerShell、Linux/BSD では `zenity` / `kdialog` / `xmessage` のうち最初に見つかったものを使います。どれも無い環境ではログだけが残ります。

### 署名

ローカルでは ad-hoc 署名 (`-`) を使うので、追加の設定は要りません。配布用に Developer ID で署名する場合は環境変数で ID を渡します。

```sh
CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" make bundle
```

ad-hoc 以外の ID を指定したときは hardened runtime (`--options runtime`) とタイムスタンプが自動で付き、公証をそのまま通せる状態になります。entitlements が必要になったら `script/entitlements.plist` を置けば署名時に読み込まれます。

## CI

GitHub Actions (`.github/workflows/ci.yml`) で、`main` への push と pull request に対して次を実行します。

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`
- `cargo build --all-features`
