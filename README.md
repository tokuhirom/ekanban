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

期限、タグ、検索、Markdown、通知、複数ユーザー、クラウド同期は MVP の対象外です。期限、タグ、検索、アーカイブ、Undo/Redo、非同期保存、複数ボード、キーボード操作、エラー表示改善は先行して実装済みで、引き続き拡張中です。設計判断の記録は [設計の記録](docs/DESIGN.md) を参照してください。

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
  next_checklist_item_id

app_state
  key
  value

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
15. 操作単位の Undo/Redo（`Cmd+Z` / `Cmd+Shift+Z`、macOS 以外は `Ctrl`）
16. 保存の非同期化（保存中の操作キュー、完了・失敗通知、失敗時のロールバック）
17. 複数ボード（サイドバー切り替え、追加・名前変更・削除、最後に開いたボードの記憶）
18. キーボード操作（矢印キーによるカード選択、`Enter` 編集、`Delete` 削除、`Cmd+Option+矢印` によるカード移動）
19. エラー種別に応じた通知表示、フォーム項目ごとの入力注記、SQLite エラー原因の表示
20. ネイティブメニューバーと `Cmd` 系ショートカット、`.app` バンドルと署名
21. 常用に耐えるボード表示（カラム内の縦スクロール、端での自動スクロール、カラムごとのカード追加、ウィンドウ状態とフィルターの復元、システム外観に追従する配色）
22. チェックリスト、カードのコピー、カードのコンテキストメニュー、カード番号表示
23. ボードの JSON / Markdown 書き出し、SQLite バックアップ、データベース場所の表示
24. カラム・カード操作のオーバーフローメニュー、ダブルクリック編集、危険操作の確認、macOS 用アイコン

次に実装するもの:

**進行中の作業は [GitHub issues](https://github.com/tokuhirom/ekanban/issues) で管理しています。** 作業内容と受け入れ条件は各 issue にあります。

決めたことと決めなかったこと（引き継ぐ設計の決まりごと、スコープ外、実装しないと判断したもの、変更の完了条件）は [設計の記録](docs/DESIGN.md) にまとめています。

### キーボードショートカット

| 操作 | macOS | その他の OS |
| --- | --- | --- |
| カードを追加 | `Cmd+N` | `Ctrl+N` |
| カードを選択 | 矢印キー | 矢印キー |
| 選択カードを編集 | `Enter` | `Enter` |
| 選択カードを削除 | `Delete` | `Delete` |
| 選択カードをカラム間・カラム内で移動 | `Cmd+Option+矢印` | `Ctrl+Alt+矢印` |
| 元に戻す / やり直す | `Cmd+Z` / `Cmd+Shift+Z` | `Ctrl+Z` / `Ctrl+Shift+Z` |
| ボード一覧の表示を切り替え | `Cmd+Ctrl+S` | `Super+Ctrl+S` |

macOS の `Cmd` は、ほかの OS では `Ctrl` になります。例外は `Cmd+Ctrl+S` のように `Ctrl` を含む組み合わせで、こちらは `Super+Ctrl` のままです（`Ctrl` が重なると保存や検索の割り当てと衝突するため）。

### クイックキャプチャのショートカット

ekanban メニューの「クイックキャプチャのショートカット…」から、アプリが前面にいなくても効くグローバルホットキーを割り当てられます。

対応しているのは **macOS と、X11 のセッションの Linux / BSD** です。それ以外の環境ではメニューの項目が灰色になり、理由が文言に出ます。

- Wayland にはアプリから使えるグローバルホットキーの共通の仕組みがありません。X11 のセッションで起動してください
- Windows は今のところ対象外です

- **既定では無効です。** 自分で割り当てるまで、ホットキーは 1 つも登録しません。グローバルホットキーは全画面でその組み合わせを奪うため、断りなく取りません
- 設定を選ぶと帯が出るので、割り当てたい組み合わせをそのまま押してください。修飾キーを 1 つ以上含める必要があります
- 「解除する」で無効に戻ります
- ほかのアプリが既に押さえている組み合わせは登録できません。その場で理由が出て、以前の割り当てが残ります
- 常駐はしないので、ホットキーが効くのは ekanban が起動している間だけです

ホットキーを押すと、1 行入力だけの小さいウィンドウが画面中央に出ます。

- 入力欄にフォーカスがある状態で開き、上にキャプチャ先（「ボード名 / カラム名」）が出ます
- `Enter` でキャプチャ先のカラムの末尾にカードが増え、ウィンドウが閉じます。`Escape` は捨てて閉じます
- どちらもフォーカスは直前のアプリに戻ります（ボードのウィンドウが前面だった場合はそちらに戻ります）
- 入力できるのはタイトルだけです。期限やタグを付けたくなったらボードを開いてください
- 保存に失敗したときはウィンドウを閉じず、入力を残したままエラーを出します
- ウィンドウの位置とサイズは覚えません。毎回中央に出ます

キャプチャ先はカラムの `…` メニューの「クイックキャプチャ先にする」で決めます。設定画面はありません。

- 既定は、最後に開いていたボードの先頭カラムです
- キャプチャ先のカラムにはヘッダに「⚡ クイックキャプチャ先」と出ます
- キャプチャ先はアプリ全体で 1 つです。**ボードを切り替えても変わりません。** 別のボードのカラムを指している場合、そのボードを開いていなくてもそこに入ります（その場合、そのカードは Undo の対象になりません）
- キャプチャ先のボードやカラムを削除したときは、黙って既定に戻ります

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
| `make icon` | macOS 用の `assets/icon.icns` を `assets/icon.png` から生成する |
| `make bundle` | リリースビルドから `target/release/bundle/Ekanban.app` を作る |
| `make open` | `.app` を作って起動する |
| `make install` | `.app` を `/Applications` にコピーする |

`cargo build` が作るのは実行ファイルだけで、`.app` バンドルにはなりません。Dock のアイコンやアプリ名、Launchpad からの起動を正しく扱うには `make bundle` を使ってください。バンドル生成の実体は `script/bundle-mac` です。

`make bundle` は `assets/icon.png` から `assets/icon.icns` を生成し、アイコンとして取り込みます。生成には macOS の `sips` と `iconutil` が必要です。

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
