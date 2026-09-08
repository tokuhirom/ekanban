# 0040. SQLite には、盤面を文書として置く

- 日付: 2026-09-08
- 状態: 草案（未確定）
- 関連: [0036](0036-one-model-two-places-to-put-it.md)（これを置き換える）、[0039](0039-the-board-model-moves-to-typescript.md)、[0011](0011-due-counts-in-the-board-list.md)

## 状況

[0039](0039-the-board-model-moves-to-typescript.md) で盤面のモデルを TypeScript に移すと、**Rust は盤面の形を知る必要がなくなる**。知らないほうがよい、とまで言える——知れば、その形は TypeScript と Rust の 2 か所に書かれることになり、`ts-rs` による生成をやめた意味が消える。

いまの SQLite は関係スキーマ（`boards` / `columns` / `cards` / `tags` / `card_tags` / `checklist_items` / `card_events` / `app_state`、スキーマ版 12）で、`db/mod.rs` の 1,310 行が読み込み・**差分保存**・移行を持っている。差分保存は「自分の知らない行を消す」ので、2 プロセスが同じファイルを開くと片方の追加が黙って消える。[0004](0004-one-process-per-database.md) がファイルロックを置いた理由がこれである。

## 決定

**ボード 1 つを、JSON の 1 列にする。** スキーマ版 13。

```sql
CREATE TABLE boards (
    id   INTEGER PRIMARY KEY,
    rev  INTEGER NOT NULL,   -- 保存のたびに +1。競合の検出に使う
    data TEXT    NOT NULL    -- 盤面まるごと（名前・並び順・カラム・カード・タグ・アーカイブ）
);

CREATE TABLE card_events (   -- 追記専用。形は変えない
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id INTEGER NOT NULL, card_id INTEGER NOT NULL, kind TEXT NOT NULL,
    from_column_id INTEGER, to_column_id INTEGER, at INTEGER NOT NULL
);

CREATE TABLE app_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);  -- いまのまま
```

- **`data` の中身を Rust は解釈しない。** ボードの名前も並び順も `data` の中にあり、一覧は TypeScript が全ボードの `data` を読んで組む
- **履歴だけは行のまま残す。** 追記専用で形が小さく（6 つの値）、あとで SQL で数えたいものだから（`docs/DESIGN.md`「前提が揃えば検討するもの」の履歴の可視化）。Rust が知る盤面の形はこの 6 つだけになる
- 保存は 1 トランザクション——変わったボードの `data` を upsert、消えたボードを delete、履歴を追記、`app_state` を更新、`rev` を +1
- **`rev` が手元のものと合わなければ、書かずに `Conflict` を返す。** 呼んだ側が読み直す
- スキーマ 12 からの移行は**片道**。関係スキーマを 1 度だけ読んで `data` に書き出す。移行の前に必ず控えを 1 つ取る

## 理由

**盤面の形の出どころが 1 つになるから。** Rust が `data` を素通しするなら、形を書いてあるのは TypeScript だけになる。`ts-rs` の生成と `make types-check` は、形が 2 か所にあることの手当てだった。1 か所になれば手当ても要らない。

**差分保存の危うさが消えるから。** 1 列を丸ごと書けば「知らない行を消す」が起きない。[0004](0004-one-process-per-database.md) のファイルロックはこれからも要る（同時に開けば `rev` の取り合いになる）が、**黙って消える**という失敗の形は無くなり、`Conflict` として見える失敗になる。

**大きさが問題にならないから。** 数百枚のカードで数百 KB。[0018](0018-rust-owns-the-board-state.md) は同じ量を**操作のたびに IPC で往復させる**ことを既に受け入れており、書き込みはローカルの SQLite でミリ秒単位である。書く単位をボードにしておけば、1 枚のカードを直したときに書き直すのは、そのボードだけで済む。

**履歴を行のまま残す**のは、性質が違うからである。盤面は「いまの状態」で丸ごと置き換わるが、履歴は追記しかされない。同じ扱いにすると、カードを 1 枚動かすたびに履歴の全件を書き直すことになる。

## 採らなかった案

- **関係スキーマを維持し、Rust が受け取った JSON を行に落とす。** `sqlite3` から SQL で覗ける利点は残るが、盤面の形を Rust の構造体に写すことになる。生成をやめた直後に、手で写した形が 2 か所に並ぶ——`ts-rs` が防いでいたずれが、防ぐ仕組みなしで戻ってくる
- **SQLite をやめて JSON ファイルにする。** 盤面と履歴を 1 トランザクションで書けなくなる。既存ユーザーのファイルがそのまま開けること、控え（`backups/`）が 1 ファイルのコピーで済むことも失う。**器としての SQLite には、中身の形と関係なく値打ちがある**
- **`data` を JSONB や BLOB にする。** `sqlite3` から読めなくなる度合いが増えるだけで、この規模で得るものが無い
- **文書全体（全ボード）を 1 列にする。** カード 1 枚を直すたびに全ボードを書き直すことになる。ボード単位は、`rev` で競合を見る単位としても素直である
- **移行を両方向にする（13 で書いても 12 でも読める）。** 2 つの形を同時に正しく保つことになる。控えを取ったうえで片道にするほうが、確かめる対象が 1 つで済む

## 結果

得るもの。

- Rust が盤面の形を知らない。`db/mod.rs` は 1,310 行から 350 行前後の見込みになり、`store.rs`（687 行）は置き場所が 1 つになるので消える
- 期限の件数を数える場所が 3 か所（SQL の `load_boards_as_of`・`Board::due_counts`・`StoredBoard::summary`）から 1 か所になる。[0038](0038-a-column-that-means-done.md) が「3 か所とも同じ条件に保つ」と書いた手間が消える
- 保存の競合が、黙って消えるのではなく `Conflict` として見える

引き受ける不都合。

- **`sqlite3` の CLI から、SQL でカードを探せなくなる。** `json_extract` は使えるが `SELECT * FROM cards` ほど素直ではない。`docs/DEVELOPMENT.md` のスキーマの節を書き換え、データの取り出しは書き出し（JSON / Markdown）が引き受けると書く
- **期限の件数を SQL で数えるのをやめる。** [0011](0011-due-counts-in-the-board-list.md) が「`load_boards` の 1 クエリの中で SQL が数える」と書いた実装は変わる。**件数をボード一覧の各行に出すという決定そのものは変えない**——数えるのが TypeScript になるだけである。ボードの数だけ `data` を読むことになるが、個人の盤面で数個から数十個の規模である
- **移行は片道で、古い版のアプリでは開けなくなる。** 移行の前に控えを 1 つ取り、マニュアルにそう書く
- **1 回の保存で書く量が増える。** カード 1 枚のタイトルを直しても、そのボードの `data` を丸ごと書く。同じ量を既に IPC で往復させているので新しい負担ではないが、盤面が桁違いに大きくなったときに最初に当たるのはここである
