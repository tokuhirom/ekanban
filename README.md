# ekanban

手元の SQLite ファイルだけで動く、ひとり用の Kanban ボードです。アカウント、サーバー、ネットワーク接続を必要としません。

カードをドラッグ＆ドロップしてカラム間を移動したり、カラム内の順番を変えたりする操作の気持ちよさを、いちばん大事にしています。

![ボードの全体](docs/images/board.png)

## できること

- **ドラッグ＆ドロップ** — カードのカラム間移動とカラム内の並べ替え、カラム自体の並べ替え。端に近づけると自動でスクロールします
- **カード** — タイトル、説明、期限、タグ、チェックリスト。コピー、アーカイブと復元
- **カラム** — 追加、名前変更、削除、WIP 上限の警告、期限順の並べ替え、まとめてアーカイブ
- **絞り込み** — タイトルと説明の検索（全角半角・大文字小文字は同じものとして扱います）とタグ（カード上のタグを押します）。**カードは隠さず暗くします**
- **複数のボード** — 左のボード一覧で切り替え。最後に開いていたボードを覚えます
- **元に戻す / やり直す** — 操作単位の Undo / Redo
- **クイックキャプチャ** — グローバルホットキーで 1 行入力の小窓を出し、決めたカラムにカードを足します（macOS と X11 セッションの Linux / BSD）
- **キーボード操作** — カードの選択・編集・削除・移動とショートカット
- **メニュー** — macOS はネイティブのメニューバー、Linux / Windows はヘッダの `≡`
- **持ち出し** — JSON / Markdown での書き出し、データベースのバックアップ
- **テーマ** — ライト / ダーク / システムに合わせる

日本語のタイトルや説明をそのまま入力・表示できます。IME の変換中に Enter や Escape を取り上げません。

## やらないこと

意図して実装していないものです。理由は [設計の記録](docs/DESIGN.md) にあります。

- クラウド同期、アカウント、サーバー通信
- 複数ユーザー、共有、権限管理
- 通知・リマインダー（期限は表示するだけです）
- Markdown の描画（説明はプレーンテキストのままです）
- 他ツールからのインポート

## 動作環境

- **macOS** — `.app` バンドルまで用意しています
- **Linux / BSD** — 動きます。クイックキャプチャのグローバルホットキーは X11 のセッションでだけ使えます（Wayland にはアプリから使える共通の仕組みがありません）
- **Windows** — データとログの置き場所は解決します。クイックキャプチャは対象外です。実行ファイルは配っていますが、ビルドが通ることしか確かめていません

## インストールと起動

### 配布物を落とす

[Releases](https://github.com/tokuhirom/ekanban/releases) に、タグごとの実行ファイルを置いています。

| OS | ファイル |
| --- | --- |
| macOS (Apple Silicon) | `ekanban-<版>-aarch64-apple-darwin.zip`（`Ekanban.app`） |
| Linux (x86_64) | `ekanban-<版>-x86_64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64) | `ekanban-<版>-x86_64-pc-windows-msvc.zip` |

`SHA256SUMS.txt` も一緒に置いてあります。

- **macOS の `.app` は ad-hoc 署名しかしていません。** 初回は右クリックから「開く」を選んでください。ダブルクリックだと Gatekeeper に止められます。Intel Mac 向けは出していません
- **Linux のバイナリは Ubuntu 24.04 でビルドしています。** glibc 2.39 以降と、Vulkan のドライバが要ります

### ソースからビルドする

[Rust toolchain](https://www.rust-lang.org/tools/install) が必要です。SQLite は同梱されるので、別途の用意は要りません。

```sh
git clone https://github.com/tokuhirom/ekanban.git
cd ekanban
make run
```

macOS では `.app` にすると、Dock のアイコンとアプリ名が正しく出ます。

```sh
make open      # .app を作って起動する
make install   # .app を /Applications に入れる
```

`cargo build` が作るのは実行ファイルだけで、`.app` にはなりません。`make help` でタスクの一覧が出ます。

## データの置き場所

OS ごとの標準の場所に保存します。

| OS | データベース | ログ |
| --- | --- | --- |
| macOS | `~/Library/Application Support/ekanban/ekanban.sqlite3` | `~/Library/Logs/ekanban.log` |
| Linux/BSD | `$XDG_DATA_HOME/ekanban/` または `~/.local/share/ekanban/` | `$XDG_STATE_HOME/ekanban/` または `~/.local/state/ekanban/` |
| Windows | `%APPDATA%\ekanban\` | `%LOCALAPPDATA%\ekanban\` |

場所を忘れたときは、ヘルプメニューの「データベースの場所をFinderで開く」から開けます。別のファイルを使いたいときは `EKANBAN_DATABASE` に絶対パスを渡してください。

```sh
EKANBAN_DATABASE=/tmp/試し.sqlite3 make run
```

バックアップは、ヘルプメニューの「データベースをコピー…」から取れます。アプリを止めずに取っても壊れた控えにはなりません。

## もっと詳しく

| 読みたいもの | 場所 |
| --- | --- |
| 使い方（画面の見方、ショートカット、困ったとき） | [マニュアル](docs/MANUAL.md) |
| なぜそう作ってあるか（決めたこと、決めなかったこと） | [設計の記録](docs/DESIGN.md) |
| 開発（構成、データモデル、ビルドと署名、CI） | [開発の手引き](docs/DEVELOPMENT.md) |
| 進行中の作業 | [GitHub issues](https://github.com/tokuhirom/ekanban/issues) |

## ライセンス

[MIT License](LICENSE)
