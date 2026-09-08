# 0039. 盤面のモデルを TypeScript に移し、Rust は殻と置き場所に絞る

- 日付: 2026-09-08
- 状態: 草案（未確定）
- 関連: [0017](0017-moving-the-ui-to-tauri.md)、[0018](0018-rust-owns-the-board-state.md)（これを置き換える）、[0040](0040-the-shape-and-the-store-stay-in-rust.md)、[0041](0041-one-layer-of-screen-tests.md)、[0042](0042-the-browser-build-is-the-same-typescript.md)

## 状況

[0018](0018-rust-owns-the-board-state.md) は「盤面は Rust が持ち、webview は投影にする」と決めた。理由は 1 つで、**`model.rs` の写しを TypeScript に作りたくない**ことだった。当時その判断は正しかった——移行の最中で、テスト付きの `model.rs` が既にあり、それをもう 1 つ書くのは費用に見合わなかった。

いま数えると、こうなっている。

| | 行数 |
| --- | --- |
| Rust 全体 | 15,274 |
| `crates/core/src/model.rs` | 3,765（うちテスト 1,091） |
| `crates/core/src/db/mod.rs` | 2,430（うちテスト 1,120） |
| `crates/app/src/menu.rs` | 1,257（うちテスト 439） |
| `crates/app/src/commands.rs` | 913 |
| TypeScript 全体 | 8,383 |

問題は行数そのものではなく、**その行数のうち「モデルが webview の外にあるから要る」ものが占める割合**である。数えられるものだけでも次がある。

- `crates/app/src/dispatch.rs`（362 行）——コマンド名で振り分ける表。Tauri の外から同じコマンドを呼ぶためだけにある
- `crates/harness/`（514 行）——コマンドを HTTP に出す開発専用のバイナリ。Playwright から本物のモデルを触るためだけにある（[0021](0021-two-layer-testing-for-the-webview.md)）
- `crates/web/`（351 行）と `wasm-pack` のビルド——同じコマンドを wasm に組み直す。ブラウザで本物のモデルを動かすためだけにある（[0035](0035-a-browser-build-of-the-real-core.md)）
- `crates/core/src/store.rs` の `JsonStore`——置き場所を 2 つに分ける口。上の 2 つが SQLite を積めないから要る（[0036](0036-one-model-two-places-to-put-it.md)）
- `crates/app/src/snapshot.rs`（229 行）——毎回丸ごと返すスナップショットの組み立て

**足場のほうがモデルより重い。** これらは 1 つ残らず、盤面が画面の外にあることの費用である。

境界の置き方が、画面の判断まで歪ませている。

- `due_status` は「今日」を知らないので、Rust が判定と `today` を一緒に返し、webview が手元の日付と比べてずれていたら `snapshot` を呼び直す（`snapshot.rs` のコメントと `web/src/state/day.ts`）。日付をまたいだかどうかは画面が知っていることなのに、判定だけが向こう側にある
- `filter_cards` は打鍵のたびに IPC を 1 往復して、一致したカードの ID の配列を返す
- `windowTitle` の組み立てと、クイックキャプチャの入れ先の既定（`captureColumn`）も Rust が返している。どちらも「TypeScript にもう 1 つ書かせない」ためで、置き場所として自然だからではない

そして**盤面の判断は既に両側にある**。落とす位置を決めるのは `web/src/board/dnd.ts` と `keyboard.ts`（[0020](0020-pointer-based-drag-and-drop.md)）で、そこが決めた「どのカラムの何枚目か」を Rust の `move_card` が受け取っている。真実が 1 つという建前は、いちばん難しいところで既に折れている。

## 決定

**盤面のモデルを TypeScript に移す。Rust に残すのは、Tauri の殻と、置き場所と、環境が答えることだけにする。**

線引きの基準を 1 つ決める。**このアプリをウェブアプリとして作ったとして、サーバ側に書くだろうものだけを Rust に置く。** 残りはブラウザ側、つまり webview に置く。Tauri の殻は、この見立てでは「ブラウザそのもの」にあたる——ウェブアプリなら書かずに済んだはずのもの（窓、ネイティブのメニュー、OS のダイアログ、グローバルホットキー）が、殻を自分で持っているぶんだけ残る。

この基準は、あとから出てくる境界の問いにも同じ答えを出す。「打った期限をどう読むか」はサーバに置かない。「保存された盤面が壊れていないか」はサーバが見る。

TypeScript（`web/src/model/`）が持つもの。

- `Board` / `Column` / `Card` / `Tag` / `ChecklistItem` の型と、その操作（追加・更新・移動・複製・削除・アーカイブと復元）
- 採番（ボードごとの ID 名前空間を含む）と `position` の振り直し
- Undo / Redo のスタックと操作の巻き戻し
- カードの履歴（`created` / `moved` / `archived` / `restored` / `deleted`）の生成
- 検索の正規化とカード番号の読み取り、タグでの絞り込み
- 期限の解釈（`9/12` `明日` `金` `+3`）と状態の判定
- 書き出し（JSON / Markdown）の組み立て
- クイックキャプチャの割り当ての**組み立てと検証**——押されたキー（`event.code`）から `ctrl-shift-n` の形を作り、修飾キーが 1 つも無い割り当てを断るところまで。**OS に登録するのは Rust**（`shortcut.rs` は Tauri の表記への変換と登録だけになる）
- メニューの構成（[0043](0043-the-menu-is-described-by-the-webview.md)）と、ウィンドウのタイトルの組み立て

Rust に残るもの。

- **Tauri の殻**——窓と矩形の保存、ネイティブのメニュー、OS のファイル選択ダイアログ、グローバルホットキー、キャプチャの窓
- **置き場所**——SQLite（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）、日次バックアップ、OS ごとのパス、1 プロセス 1 データベースのロック、クラッシュログ
- **書き込みの検証**——保存を頼まれた盤面が壊れていないかを見る（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）。サーバは受け取ったものを検めるので、ここは Rust に置く。**盤面の判断をやり直すのではなく、整合だけを見る**
- **環境が答えること**——動いている OS、開いてよい URL かの判定（`openable_url`。webview が打った文字列を OS に渡す口なので、最後の砦は Rust に残す）、ファイルを書く、場所を開く、ログに落とす

コマンドの形が変わる。盤面を変えるコマンド（`add_card` から `set_column_done` まで 25 個）は消え、置き場所と環境の口だけになる。

- `load_board(board_id)` / `load_boards()` ——盤面と、ボードの一覧を読む
- `save_board(board, rev, events)` ——盤面と、そのとき積まれた履歴を **1 トランザクションで書く**（`docs/DESIGN.md`「盤面とカード」）。手元の版（`rev`）が合わなければ書かずに `Conflict` を返す（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）
- `create_board` / `delete_board` ——ボードの増減。ID の名前空間を切るのは置き場所の仕事なので、ここは残る
- `save_app_state(entries)` ——付随する表示の状態（テーマ、絞り込み、サイドバー、窓の矩形、キャプチャ先）

**[0018](0018-rust-owns-the-board-state.md) が決めたことのうち、次の 2 つはそのまま残す。**

- **保存が成功してから画面を差し替える。** モデルへの適用をメモリで先に見せて、失敗したら巻き戻す形にはしない。適用した文書を保存し、成功したら差し替える
- **確定していない入力（下書き）は webview が持つ**（[0032](0032-committing-a-card-field-by-field.md)）

**移すのは振る舞いだけで、形と置き場所は Rust に残す**（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）。`model.rs` はデータ定義だけになり、`ts-rs` が TypeScript の型を生成し続ける。SQLite のスキーマも `db/mod.rs` も変えない。

## 理由

**もう「書き直し」ではなく「移植」だから。** [0018](0018-rust-owns-the-board-state.md) が高すぎると見たのは「TypeScript にもう 1 つ書く」費用で、写しが 2 つ残る前提だった。いまやるのは 1 つを移して Rust 側を消すことなので、写しは残らない。移す先の仕様は既に書かれている——`model.rs` の 1,091 行のテストが、`moves_card_to_another_column` のような名前で振る舞いを 1 つずつ押さえている。これを Vitest に移せば、移植の正しさはテストが見る。

**足場がモデルより重くなったから。** 上に並べた 2,000 行あまりは、どれも「盤面が画面の外にある」ことだけを理由に存在する。モデルを内側に移すと、この足場は消えるのであって、TypeScript に移し替わるのではない。減るのは Rust の行数だけではなく、**開発の入口の数**でもある——いま画面のテストを 1 つ回すのに `cargo build` が要り、ブラウザ版を確かめるのに `wasm-pack` が要る。

**境界を越えることが、画面の判断を歪めているから。** 「今日が何日か」も「打鍵のたびに絞り込む」も画面の話で、Rust に置いたのは正しさのためではなく、写しを作らないためだった。写しの心配が消えれば、置き場所は自然なほうに戻せる。

**Rust に残るものが、Rust でしかできないことになるから。** ファイルロック、OS のパス、ネイティブのメニュー、グローバルホットキー、SQLite のトランザクション。この線引きは説明が要らない。「盤面のモデルが Rust にある」は、いまや経緯でしか説明できない。

## 採らなかった案

- **表示に近いものだけ TypeScript へ移す**（期限の表示判定、書き出しの組み立て、メニューの構成）。Rust は 2〜3 千行減るが、`ts-rs` も `dispatch` も harness も wasm も全部残る。重さの本体は行数ではなく境界の位置なので、これでは目的を達しない
- **モデルは Rust に残し、wasm を webview に直接読ませて harness と `crates/web` を畳む。** 足場は減るが、型の生成とスナップショットの往復は残り、`cargo` と `wasm-pack` が画面のテストの前提であり続ける。SQLite を積めない側の制約（[0036](0036-one-model-two-places-to-put-it.md)）もそのまま残る
- **`tauri-plugin-sql` で webview から SQL を書く。** [0018](0018-rust-owns-the-board-state.md) が断ったのと同じ理由で採らない。スキーマの移行、1 プロセス 1 データベースのロック（[0004](0004-one-process-per-database.md)）、日次バックアップ（[0003](0003-daily-backup-generations.md)）は置き場所の側の仕事で、これを webview に持ち込むと OS の話が画面に混ざる。SQL は `crates/core/src/db/` に残す
- **Rust をやめて Electron などに移る。** 配布物の大きさ、起動時間、`unsafe_code = "forbid"`、3 OS のビルド——[0017](0017-moving-the-ui-to-tauri.md) が Tauri を選んだ理由は 1 つも変わっていない。薄い殻としての Tauri はよく仕事をしている
- **モデルを両方に置き、Rust 側を「正」として突き合わせ続ける。** 移植の期間だけならそうするが（下記「結果」）、恒久的にやると 2 つの実装を永久に同期させることになる。[0021](0021-two-layer-testing-for-the-webview.md) が避けたかったものそのものである

## 結果

得るもの。

- Rust は 15,274 行から 7,000〜8,000 行の見込みになる。`crates/harness` と `crates/web` が消え、`dispatch.rs` と `export.rs` と `JsonStore` と `snapshot.rs` の大半が消え、`model.rs` はデータ定義だけになる。**`db/mod.rs` の 2,430 行と型の生成は残る**（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）
- 画面のテストが `npm` だけで回る（[0041](0041-one-layer-of-screen-tests.md)）。ブラウザ版が `wasm-pack` なしで作れる（[0042](0042-the-browser-build-is-the-same-typescript.md)）
- 盤面の判断が 1 か所に戻る。落とす位置を決める `dnd.ts` と、それを受ける `move_card` が、同じ言語の隣り合った関数になる
- 打鍵ごとの絞り込みと、日付をまたいだときの再取得が、IPC を通らなくなる

引き受ける不都合。

- **`unsafe_code = "forbid"` が守る範囲が狭くなる。** 盤面の論理は `tsc --strict` と ESLint の型付き規則に移る（`docs/DESIGN.md`「テスト」の既定どおり）。ファイルを通すために規則を切らないことが、いままでより重い意味を持つ
- **移植の間だけ、真実が 2 つある。** 段階を踏むあいだ Rust 側の実装が残る。移す単位ごとに、同じテストを両側で回してから Rust 側を消す
- **Undo / Redo の移植がいちばん危ない。** `apply_operation` の 315 行は、取りこぼしても「戻せない」という形でしか現れず、画面を見ても気づけない。テストの移植を実装より先に行う
- **形を変えるときは、いままでどおり Rust を触ることになる**（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）。振る舞いだけなら TypeScript で完結するが、フィールドを 1 つ足すには `make types` を回す
- **2 つの窓（ボードとキャプチャ）が同じボードを書く競合が、新しく出てくる。** いままでは Rust の `AppState` が 1 つだったので起きなかった。`boards.rev` で検出し、負けたほうが読み直す（[0040](0040-the-shape-and-the-store-stay-in-rust.md)）
