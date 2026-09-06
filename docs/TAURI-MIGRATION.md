# Tauri 移行の設計

[ADR 0017](adr/0017-moving-the-ui-to-tauri.md) で UI を gpui-kit から Tauri へ移すと決めました。決まったのは**方針**だけなので、この文書で**いまの機能をどう実現するか**を決めます。

**この文書は移行が着地するまでの地図です。** 着地したら消します。そのとき、守るべき規則は [`docs/DESIGN.md`](DESIGN.md) へ、判断の経緯は [ADR](adr/README.md) へ移り、ここには何も残りません。

| | 読むもの |
| --- | --- |
| [`docs/DESIGN.md`](DESIGN.md) | **いま従うべき規則**。移行が着地するまで、gpui 由来の行も有効なまま |
| [`docs/adr/`](adr/README.md) | **判断の経緯**。この移行のぶんは [0018](adr/0018-rust-owns-the-board-state.md) [0019](adr/0019-typescript-react-vite-for-the-webview.md) [0020](adr/0020-pointer-based-drag-and-drop.md) [0021](adr/0021-two-layer-testing-for-the-webview.md) |
| この文書 | **移行の設計**。何をどこに置き、どの順で作り、どこで撤退するか |

---

## 1. 全体の形

3 層に分けます。**境界の引き方が、この移行でいちばん効く判断**です。

```
crates/core/   ekanban-core     gpui にも tauri にも依存しない。model / db / backup / paths / instance / diagnostics
crates/app/    ekanban          Tauri のバイナリ。command / event / menu / window / global shortcut
crates/harness/ ekanban-harness 開発とテスト専用。core を HTTP に出してブラウザから叩けるようにする（§10）
web/                            画面。TypeScript + React + Vite（ADR 0019）
```

`crates/core` は**いまの `src/` からそのまま持っていく 6,391 行**です。`gpui` という文字列が 1 つも出てこないので、移すのはファイルの場所と `serde` の derive だけになります。ここに `tauri` を依存させないことを、`crates/core/Cargo.toml` で担保します。テストが Tauri のランタイムなしで走り続けること、ハーネス（§10）が同じコードを再利用できることが、これで決まります。

依存の増減はこうなります。

| | いま | 移行後 |
| --- | --- | --- |
| 残る | `rusqlite`（bundled）、`chrono`、`thiserror`、`serde_json` | 同じ。SQL は `crates/core/db/` に閉じたまま |
| 差し替え | `global-hotkey` を直接 | `tauri-plugin-global-shortcut`（中身は同じ tauri-apps のクレート。[ADR 0012](adr/0012-focus-after-quick-capture-on-linux.md) の制約もそのまま） |
| 消える | `gpui-kit`、`lsp-types`（説明欄をコードエディタで持つために入っていた） | — |
| 足す | — | `tauri`、`tauri-plugin-dialog`（ファイル選択）、`tauri-plugin-opener`（場所を開く）、`serde`、`ts-rs`（型の生成）、Node と npm |

`tauri-plugin-sql` は**使いません**。スキーマ移行も差分保存も `crates/core/db/` にあり、それを捨てる理由がありません。`tauri-plugin-store` も使いません。表示の状態は `app_state` テーブルに入っていて、データの置き場所を 2 つに割る理由がありません。

---

## 2. 状態は Rust が持つ（[ADR 0018](adr/0018-rust-owns-the-board-state.md)）

**`Board` は Rust 側の `State` が持ち、webview はその投影だけを描きます。** 盤面の論理（採番・並べ替え・Undo / Redo・アーカイブ・タグ）は `model.rs` の 3,386 行がすでに持っていて、テストも付いています。同じものを TypeScript にもう 1 つ持つと、移行のたびに 2 つの真実を突き合わせることになります。

```rust
struct AppState {
    database_path: PathBuf,
    board: Mutex<Board>,      // 開いているボード。Undo / Redo のスタックもこの中
    save: Mutex<()>,          // 保存の直列化。いまの save_lock がそのまま残る
}
```

### コマンドは新しい盤面を返す

**盤面を変えるコマンドは、変更後のスナップショットを丸ごと返します。** 差分は返しません。ボードは数百枚のカードで、JSON にして数百 KB。操作は「1 回の確定」ごとにしか起きない（打鍵ごとには起きない）ので、これで足ります。差分にすると、適用の順序と欠落を webview 側で面倒みることになり、そこは今回いちばん作りたくない部分です。

```ts
type Snapshot = {
  board: Board;              // columns / cards / tags / archived_cards
  boards: BoardSummary[];    // 期限の件数つき。サイドバーがこれを描く
  canUndo: boolean;
  canRedo: boolean;
};
```

大きさが問題になったら、そのときに `move_card` のような高頻度のものだけ差分に落とします。**先に測ってから決めます**（§13）。

### 保存はコマンドの中で終わらせる

いまは「メモリを先に変える → 非同期で保存 → 失敗したら巻き戻す」で、そのために `PendingSave` / `ActiveSave` / `SaveFailure` の 3 つと、編集中のエディタを復元する 6 分岐があります。移行後はこうします。

**コマンドの中で、モデルの適用と SQLite への保存を続けて行い、両方成功してからスナップショットを返します。** 失敗したらモデルへの変更も捨てて `Err` を返します。画面はまだ何も変えていないので、巻き戻すものがありません。`SaveFailure` の 6 分岐と `PendingSave` / `ActiveSave` は消えます。

Tauri のコマンドは webview のイベントループを止めないので、ローカル SQLite の書き込み（ミリ秒未満〜数ミリ秒）を待っても画面は固まりません。D&D だけはドラッグ中の追従を webview 側で完結させ（§6）、`drop` の 1 回だけコマンドを呼びます。

### 下書きは webview が持つ

**確定していない入力は webview のものです。** タイトル、説明、タグ名、カラム名、検索語。IME の変換中の状態を IPC 越しに往復させる意味がありませんし、`Escape` で捨てるだけのものを Rust に持たせる理由もありません。

これで**「まだ保存していないカード」という状態が消えます。** いまは `add_card` が先にモデルへカードを足し、タイトルが入るまで保存を保留し、キャンセルされたら `discard_added_card` で取り下げています（`docs/DESIGN.md`「新しいカードは、保存されるまで足さない」）。移行後は、下書きが webview の中にあるだけなので、`add_card` はタイトルが入って確定した瞬間に 1 回呼ばれます。規則の意図（無題のカードを作らない・取り下げを `card_events` に残さない）は、経路が消えることでそのまま満たされます。

### 誰が何を持つか

| 種類 | 置き場所 | 例 |
| --- | --- | --- |
| 盤面 | Rust（`AppState`） | カード、カラム、タグ、アーカイブ、Undo / Redo のスタック |
| 永続する表示の状態 | SQLite の `app_state`（Rust 経由） | 絞り込み、最後に開いたボード、テーマ、サイドバー、ウィンドウ矩形、キャプチャ先と割り当て |
| 一時的な表示の状態 | webview | 開いているパネル、選択中のカード、メニューの開閉、スクロール位置 |
| 下書き | webview | 入力欄の中身。確定するまで Rust に渡さない |

---

## 3. コマンドとイベント

コマンドの名前は `model.rs` / `Database` のメソッドに揃えます。**1 つのコマンドが 1 つのモデル操作**を呼び、保存し、スナップショットを返す形を崩しません。

| 区分 | コマンド | 返すもの |
| --- | --- | --- |
| 起動 | `load_startup_state` | 開くボード、ボード一覧、絞り込み、矩形、テーマ、サイドバー、割り当て、キャプチャ先 |
| ボード | `create_board` `rename_board` `delete_board` `switch_board` | `Snapshot` |
| カード | `add_card` `update_card` `move_card` `copy_card` `delete_card` `archive_card` `restore_card` `set_card_due_date` `set_card_tags` | `Snapshot` |
| チェックリスト | `update_card`（項目ごと一括で受ける。`update_card_details_with_checklist` がすでにその形） | `Snapshot` |
| カラム | `add_column` `rename_column` `remove_column` `move_column` `set_column_wip_limit` `sort_column_by_due_date` `archive_column` | `Snapshot` |
| タグ | `add_tag` `rename_tag` `set_tag_color` `remove_tag` | `Snapshot` |
| 取り消し | `undo` `redo` | `Snapshot` |
| 絞り込み | `filter_cards`（検索語とタグ） | 一致したカードの ID（§5） |
| 表示の状態 | `set_filter_state` `set_theme_preference` `set_sidebar_collapsed` `set_window_bounds` | `()` |
| ファイル | `export_board`（json / markdown）`backup_database` `reveal_database` `reveal_backups` `database_location` | 書けたパス |
| キャプチャ | `capture_card` `set_capture_target` `set_quick_capture_shortcut` `close_capture_window` | `Snapshot` / `()` |
| 記録 | `log_frontend_error` | `()` |

イベントは Rust から webview への一方向で、3 つだけにします。

| イベント | いつ | 受け取ってすること |
| --- | --- | --- |
| `board:changed` | クイックキャプチャが書いたとき、ほかのウィンドウが盤面を変えたとき | スナップショットを差し替える |
| `app:action` | メニューが押されたとき | フロントの dispatcher に流す（§7） |
| `capture:result` | キャプチャの保存が終わったとき | キャプチャウィンドウを閉じる / 失敗を出す |

### 失敗の伝え方

コマンドは `Result<T, AppError>` を返します。[ADR 0016](adr/0016-where-the-app-says-things.md)（アプリが伝えることを行き先ごとに分ける）はそのまま生き、**判断の材料をコマンド側が付けて返します**。

```rust
struct AppError {
    kind: ErrorKind,   // Save / BoardIo / Export / Shortcut / Validation
    title: String,     // 「保存に失敗しました」
    detail: String,    // 使う人が手を打てる言葉に直したもの
    field: Option<Field>, // 入力欄に返す場合だけ（期限の書式、WIP 制限）
}
```

`Validation` は入力欄の脇に出し、それ以外はダイアログに出します。いまの `ErrorContext` / `FieldError` / `db_error_detail` の対応表がそのまま移ります。**拒否・キャンセル・変更なしは、いまと同じく何も言いません。**

### 境界を越える値

| 値 | 形 | 注意 |
| --- | --- | --- |
| ID（`i64`） | JSON の数値 | ID は `board_id << 32` で名前空間を切っているので、ボードが 2^21（約 209 万）を超えると JavaScript の安全な整数（2^53）から外れます。実用上は当たりませんが、**当たったら黙って壊れる**ので、`crates/core` に「ID は 2^53 未満」を確かめるテストを置きます |
| 期限（`NaiveDate`） | `"YYYY-MM-DD"` の文字列 | 時刻を持たない規則をまたいでも壊さないため。`Date` にしない（タイムゾーンで日付がずれる） |
| 時刻（`created_at` 等） | epoch **ミリ秒**の数値 | 表示のときだけ webview がローカル時刻に直す。`db` の `now()` が `as_millis` なので、秒だと思って読むと 1970 年が出る |
| 型定義 | `ts-rs` で Rust から生成する | 手で 2 か所に書くと必ずずれます。生成物の差分を CI で確かめます |
| 鍵の名前 | camelCase（`#[serde(rename_all)]`） | 受け取るのは TypeScript なので、そちらの流儀に寄せます |

---

## 4. 画面の構造（[ADR 0019](adr/0019-typescript-react-vite-for-the-webview.md)）

TypeScript + React + Vite。分解はいまの `render_*` に合わせます。移すものが 1 対 1 で追えるほうが、抜けに気づけます。

```
web/src/
  ipc/         invoke の口。tauri と harness の 2 実装（§10）
  state/       スナップショットの保持と、コマンドを呼んで差し替える 1 本の経路
  shell/       メニューのアクション dispatcher、ダイアログ、テーマ、キー入力
  board/       サイドバー、ヘッダ、検索、カラム、カード、D&D
  panel/       カードの編集パネル、タグ整理パネル、アーカイブ表示
  capture/     クイックキャプチャのウィンドウ（別のエントリポイント）
```

### 色

`ActiveTheme::theme()` は CSS のカスタムプロパティに置き換えます。**`docs/DESIGN.md` の色の規則はそのまま生きます**——直書きの色はユーザーが指定したタグの色だけ、`*_foreground` は対応する背景の上でだけ使う、色だけに意味を持たせない。トークンの名前は `--color-danger` のようにいまの `UiColor` に揃え、対応を追えるようにします。

`overflow_y_scroll` と `min_h_0` の規則は、**CSS でもそのまま必要**です（flex アイテムの `min-height: auto` は gpui の作法ではなく CSS の仕様で、gpui がそれを写しているだけ）。#43 と同じ間違いは webview でも起こせるので、規則は生き残ります。

### いまの画面で作りが変わるところ

| いまの作り | 移行後 |
| --- | --- |
| 説明欄をコードエディタ（`EditorState`）で持ち、行番号・折りたたみ・検索を切っている | `textarea` と、その裏に重ねた同じ字送りの表示層。URL はその層で描き、`Cmd` / `Ctrl` + クリックの当たり判定もそこで取る。[ADR 0002](adr/0002-links-inside-the-description-field.md)（説明はプレーンテキストのまま、拾うのは `http(s)://` だけ）は変わらない。`lsp-types` の依存が消え、#89（サイズが変わらない）も解ける |
| タグの入力に相当する部品が無い | チップ列＋補完付き入力。#90 が解ける |
| ボード名の入力がインライン | ダイアログにする。#91 が解ける |
| カードパネルのメニューが入力欄の下に潜る（#78） | 重なりは CSS の積み重ね文脈で決まるので、同じ壊れ方をしない |
| メニューが Linux で反応しない（#92 #93） | OS 側のメニューになる（§7） |

### webview だから要る手当て

ネイティブでは考えなくてよかったものが、webview では自分で切ります。**移行の隠れた費用なので、先に列挙しておきます。**

- 既定の右クリックメニューを止める（カードの右クリックメニューを出すため）
- リリースビルドで devtools と再読み込み（`Ctrl+R` / `F5`）を無効にする
- ドラッグ中の文字列選択を止める（`user-select: none`）。画像とテキストの既定のドラッグも止める
- macOS の 2 本指スワイプによる履歴移動を止める
- 拡大縮小（`Ctrl+スクロール`）を、意図した場合以外は止める
- フォントはシステムのものを使う。web フォントは読み込まない（ネットワークに出ない）

---

## 5. 絞り込み・検索・アーカイブ

全部 `model.rs` に純粋関数としてあります（`normalize_search_text`、`card_matches_search`、`parse_card_number_query`、`due_status`）。**判定は Rust に残し、結果だけを webview に渡します。** 全角半角と大文字小文字の正規化を TypeScript でもう一度書くと、2 つの正規化がずれた日にカードが見つからなくなります。

検索語を打っている間に毎打鍵で IPC を往復させないよう、`card_matches_search` と同じ判定を webview でも持ちたくなりますが、**持ちません**。スナップショットに「このカードは減光か」を載せるのではなく、`filter_cards(query, tag_id)` コマンドが**一致したカードの ID 集合**を返し、webview はそれを見て減光します。打鍵ごとに呼びますが、返るのは ID の配列だけです。

- ボードでは隠さず減光する（D&D の挿入位置が曖昧にならないため）
- アーカイブ表示だけは隠し、見出しに「一致 / 全件」を出す（[ADR 0010](adr/0010-hiding-instead-of-dimming-in-the-archive.md)）
- `#12` はカード番号として読む（[ADR 0008](adr/0008-reaching-a-card-by-its-number.md)）

どれも規則のまま残ります。

---

## 6. ドラッグ＆ドロップ（[ADR 0020](adr/0020-pointer-based-drag-and-drop.md)）

README が「いちばん大事にしています」と書いているところで、[ADR 0017](adr/0017-moving-the-ui-to-tauri.md) が移行の成否をここで判定すると書いたところです。**先に受け入れ条件を決めてから作ります。**

| # | 条件 |
| --- | --- |
| 1 | 掴んだカードのゴーストがポインタに遅れずついてくる |
| 2 | 落とす位置（どのカラムの何枚目か）が、落とす前に見て分かる |
| 3 | カラムの端にポインタを寄せると自動でスクロールし、押したまま端で止めても滑らかに続く |
| 4 | 減光しているカードの位置にも落とせる（絞り込み中に挿入位置が変わらない） |
| 5 | カラムそのものの並べ替えも、カードと同じ操作感でできる |
| 6 | キーボードでもカードを動かせる（いまの `secondary`＋`alt`＋矢印） |
| 7 | 落としてから画面が確定するまでに間が空かない |
| 8 | 3 つの webview（WKWebView / WebView2 / WebKitGTK）で 1〜7 が同じ |

作りは次のとおりです。

- **HTML5 の drag events（`dragstart` / `dragover` / `drop`）は使いません。** ゴーストの見た目を OS に取られ、`dragover` の間引きも制御できず、WebKitGTK と WKWebView で挙動が揃いません。条件 1・2・8 がこれで落ちます
- ポインタイベント（`pointerdown` / `pointermove` / `pointerup` と `setPointerCapture`）で作ります。既製のライブラリ（dnd-kit）に載せるのが既定で、ゴースト・挿入位置・オートスクロール・キーボード操作を自分で書き直さない（「作りたいのは Kanban であって UI ツールキットではない」）。段階 4 の spike で 1〜8 を確かめ、届かなければポインタイベントで自前に書く
- **ドラッグ中は webview の中だけで完結**させ、`drop` の瞬間に `move_card` / `move_column` を 1 回だけ呼びます。挿入位置の計算はいまと同じで、カラムなら末尾の index、カードなら自分の index

カード表面の高さを可変にしない規則（チェックリストの件数で高さを変えない）は、条件 2 の安定のために残します。

---

## 7. メニューとキー割り当て

Tauri のメニューは 3 つの OS で**それぞれのネイティブなメニューバー**になります（macOS はアプリメニュー、Windows と Linux はウィンドウのメニューバー）。

**これで `src/views/menu_bar.rs` と `src/views/window_chrome.rs` が消えます。** [ADR 0015](adr/0015-a-menu-bar-on-every-platform.md)（どのプラットフォームでもメニューバーを出す）の**決定は残り、自分で描くという実装だけが消えます**。[ADR 0005](adr/0005-in-app-menu-without-a-menu-bar.md) は 0015 に置き換えられており、`Decorations` を見て枠を自分で描く話も、GTK が装飾を持つので不要になります。

メニューの構成（macOS とそれ以外で違う）と項目の文言は、いまの `menus()` をそのまま移します。押されたら Rust の `on_menu_event` が `app:action` を webview に投げ、**フロント側の 1 本の dispatcher** がキーボードからの経路と同じ関数を呼びます。「ボードは開いた時点でフォーカスを持つ」規則は、フォーカスに関係なく届く形になるので不要になります（#92 #93 が解ける理由でもあります）。

### アクセラレータを付けるものと、付けないもの

**ここが移行でいちばん間違えやすい**ので、規則を決めます。

> **メニューのアクセラレータは、フォーカスに関係なく効いてよいキーにだけ付ける。テキスト編集に譲らなければならないキーには付けず、webview の keydown で判定する。**

OS のメニューは webview より先にキーを取ります。`Cmd+Z` にメニュー項目のアクセラレータを付けると、**入力欄で打っている最中の取り消しまで盤面の Undo に食われます**。`docs/DESIGN.md` の「入力欄にフォーカスがある間はボードのショートカットを無効にし、IME とテキスト編集を優先する」を守るには、こうなります。

| いまの割り当て | 移行後 |
| --- | --- |
| `secondary-n` `secondary-shift-n` `secondary-shift-b` `secondary-shift-t` `secondary-f` `secondary-s` `secondary-shift-a` `secondary-shift-f` `secondary-w` | `CmdOrCtrl+…` のアクセラレータを付ける（テキスト編集と衝突しない） |
| `secondary-z` `secondary-shift-z`（Undo / Redo） | **アクセラレータを付けない。** メニュー項目は残し、キーは webview で受けて、入力欄にフォーカスがあれば webview 自身の取り消しに、無ければ盤面の Undo に振り分ける |
| カット / コピー / ペースト / すべてを選択 | Tauri の既定のメニュー項目（OS の編集操作）に任せる |
| `cmd-ctrl-s` / `secondary-b`（ボード一覧）、`cmd-ctrl-f` / `f11`（フルスクリーン）、`cmd-q` / `secondary-q`（終了）、`cmd-m`（しまう） | OS ごとの分岐はそのまま。`CmdOrCtrl` は macOS で Cmd、ほかで Ctrl になるので、[ADR 0009](adr/0009-per-platform-key-bindings.md) の「`cmd-` が Super に落ちる」問題そのものは消える。**分岐が要る理由（macOS の標準の組み合わせ）は残る** |
| `Enter` / `Escape` / 矢印 / `secondary`＋`alt`＋矢印 | webview の keydown。メニューには出さない |

---

## 8. ウィンドウ

| いまの作り | 移行後 |
| --- | --- |
| `WindowOptions` で開き、`app_id` を渡す | `tauri.conf.json` の `identifier` と `mainBinaryName`。[ADR 0013](adr/0013-linux-desktop-integration.md) の `StartupWMClass` との一致は**生成された `.desktop` を見て確かめる**（いまの `the_linux_desktop_entry_points_at_the_app_id` テストの意図を、生成物に対して置き直す） |
| 矩形を `app_state` に保存し、起動時に復元 | 同じ。`tauri-plugin-window-state` は使わない。データの置き場所を 2 つに割らないため。表示可能なモニタに載っているかの確認も、いまの `restored_window_bounds` の判定を移す |
| Wayland では位置を復元しない | 変わらない（クライアントが自分の位置を知れないのは webview でも同じ） |
| `Application::on_reopen` で Dock から開き直す | `RunEvent::Reopen`（macOS）。開き直しでデータベースから読み直す規則も変わらない |
| ウィンドウを閉じてもプロセスが残る（macOS） | 同じ挙動に合わせる |
| 装飾を `Decorations` で判定して自分で描く | 不要（§7） |
| 同じデータベースを 2 プロセスで開かせない（`instance.rs` のファイルロック） | **そのまま残す。** `tauri-plugin-single-instance` は**使わない**——あれはアプリ 1 つに対する制限で、[ADR 0004](adr/0004-one-process-per-database.md) が決めた「ロックはデータベースのパス単位」（`EKANBAN_DATABASE` を使い分けて並べて動かせる）を壊す |

---

## 9. クイックキャプチャ・ファイル・守り

### クイックキャプチャ

- 割り当ての登録は `tauri-plugin-global-shortcut`。中身は同じ `global-hotkey` なので、**使えるのは macOS と X11 だけ**という判定（`hotkey.rs` の `platform_support`）はそのまま移します。登録の戻り値を信じない理由も変わりません
- 既定では登録しない、修飾キーなしを受け付けない、登録できなかった割り当てを保存しない——全部そのまま
- **保存形式が変わります。** いまは gpui の表記（`ctrl-alt-shift-cmd-n`）で `app_state` に入っていますが、Tauri は `"CommandOrControl+Shift+N"` の形です。**スキーマ v11 で 1 回だけ変換する移行を書きます**（読めなかった値は捨てて未設定に戻す。起動を妨げない）
- 割り当ての捕捉 UI は webview の keydown で受け、`event.code` を Tauri の表記に直します
- 入力ウィンドウは 2 つ目の `WebviewWindow`（装飾なし・最前面・タスクバーに出さない・画面中央）。**保存は `capture_card` コマンドを通し、ボードと同じ保存経路に乗せます**（カラムの末尾に足す・Undo の対象になる・`created` が 1 件積まれる）
- 閉じたあとのフォーカスの戻し先は [ADR 0012](adr/0012-focus-after-quick-capture-on-linux.md) のまま。Linux で保証するのは 1 つだけ

### ファイル

| 操作 | 移行後 |
| --- | --- |
| 書き出し先・控えの保存先を選ぶ | `tauri-plugin-dialog` の保存ダイアログ（OS のネイティブ。Linux は portal 経由） |
| 書き出しの中身を作る | `crates/core`。`export_board_json` と Markdown の描画はそのまま |
| データベース / バックアップの場所を開く | `tauri-plugin-opener` の `reveal_item_in_dir` |
| 確認ダイアログと、失敗・完了の報告 | **アプリ内の `<dialog>`**（ネイティブのメッセージダイアログは使わない）。文言・ボタン・「フォルダを開く」導線を自分で決められること、Playwright から見えること（§10）の 2 つが理由。[ADR 0016](adr/0016-where-the-app-says-things.md) の規則（何をどこに出すか）は変わらない |

`cx.defer` にダイアログを載せる規則は、gpui の `update_window` の都合だったので消えます。

### 守り

- 日ごとの世代バックアップ（`backup.rs`）は無改造。起動時に別スレッドで取り、失敗しても起動を止めない
- `diagnostics.rs` のパニックフックと起動失敗の記録はそのまま。**加えて、webview の未捕捉例外を `log_frontend_error` で同じログに落とします**。webview の失敗が黙って消えると、原因を追う手段が無くなるため
- `tauri.conf.json` の CSP を `default-src 'self'` にし、capability（プラグインの許可）は使うものだけに絞ります。**ネットワークに出ないことを設定で担保します**（README の「アカウント、サーバー、ネットワーク接続を必要としない」）
- `unsafe_code = "forbid"` は Rust 側に残ります。TypeScript 側は `tsc --strict` と ESLint で代えます

---

## 10. テスト（[ADR 0021](adr/0021-two-layer-testing-for-the-webview.md)）

[ADR 0017](adr/0017-moving-the-ui-to-tauri.md) に「webview なら Playwright がそのまま使える」と書きましたが、**そのままでは使えません**。Playwright が繋がるのは Chromium / Firefox / WebKit であって、WKWebView・WebView2・WebKitGTK ではありません。Tauri の標準の E2E は `tauri-driver`（WebDriver）で、**macOS には WebDriver が無いので動きません**。

そこで 2 層にします。

| 層 | 何で | 何を担保するか | どこで走るか |
| --- | --- | --- | --- |
| 中核 | `cargo test` | モデル・SQLite・移行・バックアップ。いまの資産がそのまま | 3 OS |
| 画面 | **Playwright ＋ `ekanban-harness`** | 操作から SQLite までを通した振る舞い。`view_tests.rs` の 1,047 行の行き先 | Linux（CI）と手元 |
| 殻 | `tauri-driver`（WebdriverIO）の煙テスト | ウィンドウが開く・メニューが出る・キャプチャウィンドウが開く | Linux と Windows。macOS は手で |
| 部品 | Vitest | 日付の表示、挿入位置の計算のような純粋な部分 | Linux（CI） |

`ekanban-harness` は、**`crates/core` を同じコマンド名で HTTP に出すだけの開発用のバイナリ**です。webview は `ipc/` の口を通してコマンドを呼ぶので、口の実装を差し替えるだけで、同じ画面がブラウザでも動きます。Playwright はそのブラウザを叩き、テストは一時ファイルの SQLite を直接読んで結果を確かめます——**いまの `Harness::stored_board` と同じやり方**です。偽物のバックエンドを TypeScript で書かないので、モデルの挙動がテストの中でだけ違う、が起きません。

担保できないのは「webview の実装ごとの差」です。そこは殻の煙テストと、リリース前の手での確認（3 OS ）に残ります。**`run_until_parked()` の代わりは Playwright の待ち合わせ**（要素の出現）で、実時間の `sleep` を書かないという規則の意図はそのまま残ります。

---

## 11. ビルド・CI・配布

```
make dev      tauri dev（Vite の開発サーバごと）
make check    cargo fmt --check / clippy / test  ＋  tsc / eslint / vitest
make e2e      harness を上げて Playwright
make bundle   tauri build（3 OS のバンドラ）
```

| いま | 移行後 |
| --- | --- |
| `script/bundle-mac`（`.app` を手で組む） | Tauri のバンドラ（`.app` / `.dmg`）。[ADR 0014](adr/0014-unsigned-apple-silicon-only-macos-builds.md)（Apple Silicon 向けの未公証ビルドだけを配る）は変わらず、ad-hoc 署名の指定が `tauri.conf.json` に移る |
| `script/install-linux`（`~/.local` に入れる） | バンドラの `.deb` / `.AppImage` を配りつつ、**root を要求しない導線は残す**（[ADR 0013](adr/0013-linux-desktop-integration.md) の決定は「ユーザーのディレクトリで完結させる」ことなので、`.deb` だけにはしない） |
| `Makefile` の `icon` ターゲット（`.icns` を生成） | `tauri icon` が 3 OS ぶんを生成 |
| CI の `check`（ubuntu）＋ `cross`（macOS / Windows） | 同じ 2 ジョブ構成を保つ。**必須チェックの名前 `Check and test` は変えない**（ルールセットが名前で見ており、変えると全 PR がマージ不能になる）。ubuntu 側に Node のセットアップ、`tsc` / `eslint` / `vitest` / Playwright を足す |
| Linux の依存（X11・Vulkan 一式） | WebKitGTK 一式（`libwebkit2gtk-4.1-dev`、`libsoup-3.0-dev`、`libjavascriptcoregtk-4.1-dev` など）に入れ替え |
| リリース（tagpr → タグ → 3 OS ビルド） | 経路は同じ。成果物の名前と作り方だけ差し替え |

---

## 12. 段階

**各段階に「終わったと言える条件」を書きます。** 動くものが常にある順に並べ、gpui 側は凍結したまま触りません。

済んだ段階には ✅ を付けます。**この印が、いまどこにいるかの唯一の記録です。**

| # | やること | 終わったと言える条件 |
| --- | --- | --- |
| 0 | メモリの実測（§13） | 3 OS で、いまの値と Tauri の空アプリの値が表になっている |
| 1 ✅ | ワークスペースへ再編。`crates/core` を切り出す | `cargo test` が通り、`crates/core` が gpui にも tauri にも依存していない |
| 2 ✅ | `serde` 化、`ts-rs` の生成、コマンドとイベントの実装（画面なし） | コマンドの一覧（§3）が全部あり、Rust のテストから叩ける |
| 3 ✅ | 読むだけの盤面（サイドバー・ヘッダ・カラム・カード・減光） | 手元のデータベースを開いて、いまのアプリと同じ盤面が出る |
| 4 ⚠ | **D&D の spike** | §6 の 1〜8 を 3 OS で満たす。**満たさなければここで止めて判断し直す**（1〜7 は WebKitGTK で確認済み。条件 8 が残っている） |
| 5 | 編集（カードパネル、チェックリスト、タグ、期限、カラム、ボード） | いまの `view_tests.rs` の項目が Playwright で通る |
| 6 | メニュー、キー割り当て、テーマ、ウィンドウの状態 | §7 の表のとおりに効き、入力中の `Cmd+Z` が盤面を巻き戻さない |
| 7 | アーカイブ、書き出し、バックアップ、場所を開く | 書き出したファイルが読め、控えが `backups/` に増える |
| 8 | クイックキャプチャ（別ウィンドウ、割り当て、v11 移行） | 旧形式の割り当てが入ったデータベースを開いて、そのまま使える |
| 9 | 配布（バンドル、CI、リリース） | 3 OS の成果物が CI から出て、手元で起動する |
| 10 | 入れ替え | gpui のコードと依存を消す。`docs/DESIGN.md` の gpui 由来の行を書き換え、置き換える ADR を書く。この文書を消す |

### 段階 1 で決まったこと

`crates/gpui` という 3 つ目のクレートが要りました。§1 の表には `core` / `app` / `harness` の 3 つしか無く、**いまの gpui のアプリの置き場所が書いてありません**。ワークスペースのルートをパッケージにするとその名前が `ekanban` を先に取ってしまい、段階 9 で `crates/app` に同じ名前を付けられません。そこで gpui のアプリを `crates/gpui`（パッケージ名 `ekanban`）に置き、ルートは仮想マニフェストにしました。段階 10 でこのクレートを消すとき、`crates/app` が名前を引き継ぎます。

`crates/gpui/src/lib.rs` は `ekanban_core` のモジュールを**同じ名前で出し直しています**。凍結すると決めたコード（[ADR 0017](adr/0017-moving-the-ui-to-tauri.md)）に import の付け替えを 6,000 行ぶん入れる理由がなく、どうせ消えるクレートだからです。新しく書くほうは `ekanban_core::` を直接使ってください。

中核が UI ツールキットを引きずり込んでいないことは、`script/check-core-independence` が CI で見ています。Cargo.toml を読むだけでは足りません——依存の依存から入るほうが、手で書いて足すよりありがちなので、解決した依存グラフを見ています。

### 段階 2 で決まったこと

**コマンドの層は `tauri` に依存していません。** `crates/app`（パッケージ名 `ekanban-app`）に §3 のコマンドが普通の関数として入り、`crates/app/tests/commands.rs` が外から呼んで、返るスナップショットと SQLite の中身の両方を見ています。`#[tauri::command]` の包み・ウィンドウ・メニュー・グローバルな割り当ては、画面が出る段階 3 で足します。§10 のハーネスが同じ関数を HTTP に出す以上、**中身が Tauri を知らないことは、包みを後回しにした都合ではなく設計**です。`close_capture_window` だけは状態を持たないウィンドウ操作なので、包みと一緒に段階 3 で入ります。

**`AppState` に保存用のロックは置きませんでした。** §2 の素描は `board: Mutex<Board>` と `save: Mutex<()>` を並べていますが、コマンドは盤面のロックを持ったまま適用と保存を続けて行うので、盤面のロックが保存の順番もそのまま決めます。2 つ目のロックは同じことを 2 か所で守る形になります。

**「無題のカードを作らない」を守る場所が変わりました。** `Board::add_card` はタイトルを見ません——gpui 版が空文字で呼んでから中身を埋めていたためです。下書きが webview のものになって（§2）その経路が消えたので、**断るのはコマンドの入口**になりました。`add_card` と `capture_card` が空白だけのタイトルを `Validation` で返します。

**Markdown の書き出しを `crates/core/src/export.rs` へ移しました**（§9 のとおり）。gpui のビューの中にあると、Tauri 側からもハーネスからも呼べません。

**`i64` は TypeScript の `number` です。** ts-rs の既定は `bigint` ですが、値を書くのは `serde_json` で JSON の数値になり、`JSON.parse` が返すのも `number` です。型だけ `bigint` にすると、**実行時の値と型が食い違ったまま通ります**。`.cargo/config.toml` の `TS_RS_LARGE_INT` で 1 か所に決め、それが外れたら落ちるテストを両方のクレートに置いてあります。`number` が嘘にならないこと（ID が 2^53 に届かないこと）は `crates/core` のテストが見ています。

**時刻はエポックからのミリ秒でした**（`db` の `now()` が `as_millis`）。§3 の表を直しました。秒だと思って読むと webview は 1970 年を描きます。

### 段階 3 で決まったこと

**スナップショットに期限の状態を足しました。** §2 の Snapshot は 4 つの欄しかありませんが、カードの期限の見出し（「期限切れ 2日 (9/4)」）を描くには「今日が何日か」が要ります。`Card` はデータベースから来るものなので、その日付を持てません。判定を TypeScript に写すのは §5 が禁じているので、`due_statuses`（カード ID と `DueStatus` の対）と、それを出した `today` を snapshot が運びます。**日付をまたぐと古くなります**——開きっぱなしで日が変わると、次のコマンドまで昨日の判定が出たままです。`today` を返しているのは、webview が手元の日付とずれたことに気づいて読み直せるようにするためで、その読み直し自体はまだ作っていません。

**`crates/app` の実行ファイルは `ekanban-tauri` です。** `crates/gpui` が出す `ekanban` と並べて置ける必要があるのは移行の間だけなので、段階 10 で `ekanban` に変わります。`tauri.conf.json` の `mainBinaryName` がそこに合わせてあります。

**デバッグビルドは Vite の開発サーバを要求します。** Tauri は `devUrl` をデバッグの実行ファイルに焼き込むので、`cargo run -p ekanban-app` だけでは白い画面に「Connection refused」が出ます。`make dev`（`tauri dev`）を使ってください。

**`crates/app` のコンパイルに `web/dist` が要ります。** `tauri::generate_context!` が画面を実行ファイルに埋め込むためで、`cargo test --workspace` も同じです。CI は 3 つの OS すべてで、Rust の前に `npm ci && npm run build` を回します。§11 の表は ubuntu にだけ Node を足すと書いていましたが、`crates/app` を macOS と Windows でもコンパイルする以上、そちらにも要ります。

**TypeScript は 5.9 に留めました。** 7.0 は出ていますが `typescript-eslint` がまだ受け付けません（peer が `<6.1.0`）。§9 が `unsafe_code = "forbid"` の代わりに数えている 2 本のうち 1 本を落とすより、こちらを待ちます。

**`overflow` と `min-height: 0` の規則は、そのまま CSS でも要りました**（#43 と同じ間違い）。`.app` → `.board` → `.board-content` → `.column` → `.column-cards` の全部に `min-height: 0` が入っています。横にスクロールする `.board-content` には `min-width: 0` も要ります——軸ごとに別なので、片方だけ書くと片方が伸びます。

### 段階 4 で決まったこと（D&D の spike）

**`@dnd-kit/core` 6 系に載せます**（[ADR 0022](adr/0022-dnd-kit-core-for-drag-and-drop.md)）。§6 の受け入れ条件を WebKitGTK 上の実機で 1 つずつ測った結果です。

| # | 条件 | 結果 |
| --- | --- | --- |
| 1 | ゴーストがポインタに遅れずついてくる | ✅ `DragOverlay` に自分のカードを描いている。OS のゴーストは使わない |
| 2 | 落とす位置が落とす前に見て分かる | ✅ 並びがその場で組み替わり、WIP の数字（`3 / 2 上限超過`）も先に変わる |
| 3 | カラムの端で自動スクロールし、押したまま止めても続く | ✅ **ただし CSS を直してから**（下記） |
| 4 | 減光しているカードの位置にも落とせる | ✅ 隠さず減光しているので、挿入位置が動かない |
| 5 | カラムそのものの並べ替えも同じ操作感 | ✅ **ただし衝突判定を絞ってから**（下記） |
| 6 | キーボードでも動かせる | ✅ `Ctrl/Cmd+Alt+矢印`。gpui 版の `next_card_id` / `move_selected_card_between_columns` をそのまま移した |
| 7 | 落としてから画面が確定するまでに間が空かない | ✅ 実測 3〜24 ms（IPC ＋ SQLite の書き込み込み）。楽観更新は要らなかった |
| 8 | 3 つの webview で 1〜7 が同じ | ❌ **未確認。** WebKitGTK しか手元に無い |

**条件 8 が残っています。** [ADR 0020](adr/0020-pointer-based-drag-and-drop.md) の関門は、macOS（WKWebView）と Windows（WebView2）で 1〜7 を確かめるまで閉じません。`tauri-driver` は macOS に無いので（§10）、ここは手での確認です。

spike が見つけた、こちら側の不備が 2 つあります。

**`.board-content` に `overflow-y: hidden` を書いていませんでした。** CSS は片方の軸を `visible` 以外にすると、もう片方を `auto` に計算します。`overflow-x: auto` だけ書いてあったので、カラムが縦に伸びたときに**盤面全体が縦スクロールし、カラムの中は永久にスクロールしません**でした。条件 3 が落ちていたのはこれで、窓を高くしている間は表に出ません。#43 と同じ形の間違いです。

**入れ子の並べ替えには、衝突判定を掴んだものの種類で絞る必要がありました。** カラムの中にカードの並べ替えが入っているので、素のままだと**カラムを掴んでいるのにカードが落とし先に選ばれ**、掴んでも何も起きません。`web/src/board/Board.tsx` の `collisionDetection` がそこを絞っています。dnd-kit を外して自前に書く日が来ても、この絞り込みは要ります。

**キーボードは dnd-kit の `KeyboardSensor` を使いません。** いまの割り当ては修飾キーを押しながら矢印を叩く 1 手で、押している間に何枚でも動かせます。`KeyboardSensor` は掴む → 動かす → 離すの 3 手になり、手触りが下がります。

---

## 13. 撤退と判定

**この移行には、引き返す場所を 2 つ置きます。**

| 関門 | いつ | 判定 |
| --- | --- | --- |
| メモリ | 段階 0（着手前） | いまの ekanban と、同じ盤面を出した Tauri の試作を 3 OS で測る。[ADR 0017](adr/0017-moving-the-ui-to-tauri.md) の想定は 1.5〜2 倍。**大きく超えたら移行そのものを見直す** |
| D&D | 段階 4 | §6 の 1〜8。**届かなければ、既製のライブラリを外して自前に書くか、移行を止めるかを、そこで決める**。→ 1〜7 は `@dnd-kit/core` 6 系で満たした（[ADR 0022](adr/0022-dnd-kit-core-for-drag-and-drop.md)）。**条件 8 は未確認**——macOS と Windows の実機が要る |

測り方を決めておきます。**「起動直後」ではなく「実際の盤面を開いて 5 分触ったあと」**を測ります。webview は遅れてメモリを確保するので、起動直後の値は当てになりません。Linux の WebKitGTK は**プロセスが複数に分かれる**（UI / Web / Network）ので、**合算しないと 3 分の 1 の値を見て安心する**ことになります。macOS は `footprint`、Linux は PSS の合算、Windows は作業セットで揃えます。

---

## 14. 移行で消える規則と、生き残る規則

`docs/DESIGN.md` の行のうち、gpui の都合から来ているものは着地時に書き換えます。**判断の中身は残り、実装の話だけが消えます。**

| 消える・書き換わる | 生き残る |
| --- | --- |
| 説明欄をコードエディタのモードで持つ | 説明はプレーンテキスト。拾うのは `http(s)://` だけ。開くには修飾キーを要求する |
| 色は `ActiveTheme::theme()` から引く | 色だけに意味を持たせない。直書きはタグの色だけ。`*_foreground` の使い分け |
| `flex_col` の直下は幅いっぱいに伸びる | `overflow` と `min-height: 0` の関係（CSS の仕様なので残る） |
| `cmd-` は非 macOS で Super に落ちる | 割り当てを OS ごとに分ける。macOS 以外で platform 修飾キーを使わない |
| `Decorations` を見て枠を自分で描く／`TitleBar` を使わない | Wayland では位置を復元しない |
| `AppMenuBar` に同じ定義を読ませて自分で描く | どのプラットフォームでもメニューバーを出す |
| `cx.defer` にダイアログを載せる／`App::activate` を当てにしない | アプリが伝えることを行き先で分ける（[ADR 0016](adr/0016-where-the-app-says-things.md)） |
| 新しいカードは保存されるまで足さない（経路ごと消える） | 無題のカードを作らない。取り下げを履歴に残さない |
| `#[gpui_kit::test]` と `run_until_parked()` | 画面の振る舞いは本物のウィンドウを開いて確かめる。実時間の `sleep` で待たない |

盤面の規則（期限は日付のみ、絞り込みは減光、`position` の書き換え、履歴はライフサイクルだけ、Undo と `card_events` を分ける、1 データベース 1 プロセス、日ごとの控え、初回に消す前提のものを置かない、`Enter` で確定、常用しない操作を常時出さない、キャプチャ先はアプリ全体で 1 つ）は、**1 行も変わりません**。これが「中核を捨てない」ということの意味です。

---

## 15. 危ないところ

| | 中身 | 手当て |
| --- | --- | --- |
| WebKitGTK の差 | レンダリングも入力も、WKWebView / WebView2 と揃わない。プラットフォーム差が**別の種類に置き換わる**だけ | 段階 4 と 5 で 3 OS を都度確かめる。CI の煙テストは Linux と Windows で回す |
| Linux の描画 | ドライバによっては真っ白になる（`WEBKIT_DISABLE_DMABUF_RENDERER` の既知の回避） | 手引きに書く。起動できない報告の最初の確認項目にする |
| メモリ | 選んだ理由の 1 つを手放す | 段階 0 の関門（§13） |
| スナップショットの大きさ | カードが増えると 1 操作ごとの JSON が重くなる | 段階 5 で大きい盤面を作って測る。重ければ高頻度のコマンドだけ差分にする |
| Node と npm | いまは `cargo` だけで完結している。供給の不安定さから逃げてきたのに、依存の本数は増える | 依存を増やさない。ロックファイルを固定し、CI で監査する |
| 2 つの UI を抱える期間 | 並行して育てると倍のコストになる | gpui 側は凍結。直すのは使えなくなる不具合だけ（[ADR 0017](adr/0017-moving-the-ui-to-tauri.md)） |
| ID の桁 | `board_id << 32` が JavaScript の安全な整数を超えうる | `crates/core` に上限のテストを置く（§3） |
| macOS の E2E | `tauri-driver` が無い | 殻の確認は手に残す。中身は harness 経由の Playwright で担保する（§10） |
