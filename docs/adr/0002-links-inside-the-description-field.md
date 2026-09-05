# 0002. 説明の URL を、入力欄の中でリンクとして描く

- 日付: 2026-09-05
- 状態: 有効
- 関連: [#71](https://github.com/tokuhirom/ekanban/issues/71) [#72](https://github.com/tokuhirom/ekanban/pull/72)、[0001](0001-links-in-the-description.md) を置き換える

## 状況

[0001](0001-links-in-the-description.md) は URL を説明欄の下に一覧で出した。届いてはいたが、想定されていたのは Obsidian のような「**編集できるまま、リンクはリンクに見えて押せる**」形だった。

0001 は「詳細パネルの説明は `Textarea` で、読むだけの表示が無い」を理由に文中の装飾を諦めていた。調べ直すと、`gpui-base` の入力欄には外から色付けと当たり判定を差し込む口がある。ただし**モードによる**。

| モード | 型 | Extras |
| --- | --- | --- |
| 1 行 | `InputMode` | `()` |
| 複数行（当時の説明欄） | `TextareaMode` | **`()` — 置き場所が無い** |
| コードエディタ | `EditorMode` | `EditorExtras { lsp, decorations, hover_definition, … }` |

`LayoutMode::set_highlighter_factory` は `if let LayoutMode::CodeEditor { .. }` の中でしか代入せず、**それ以外のモードでは黙って何もしない**。`Cmd` クリックの当たり判定（`hover_definition`）と装飾の入れ物（`decorations`）も `EditorExtras` の中にある。

つまり「まず色付けだけ、クリックは後で」という分割はできない。どちらも同じ 1 つの前提を要求する。

## 決定

`CardEditor` が持つ説明を `TextareaState` から **`EditorState`（コードエディタのモード）** に変える。行番号・折りたたみ・検索は切る。そのうえで、

- `InputHighlighter` で `http(s)://` の範囲に色と下線を付ける
- `DefinitionProvider` で `Cmd` / `Ctrl` + クリックの当たり判定を返す

開くのはエディタ側がやる。`go_to_definition` は行き先の scheme が `http` / `https` なら、そのまま `cx.open_url` に渡す実装になっている。

素のクリックは取らない。**修飾キーを要求する。**

## 理由

- **モードを変えないと届かない。** 色付けもクリックも `EditorMode` にしか無い。借りるのはその 2 つの口だけで、コードとして扱うわけではない
- **修飾キーが要る理由。** 素のクリックは「そこにカーソルを置く」という編集の意味を既に持っている。奪うと編集が壊れる。Obsidian の編集モードも同じく修飾キーを要求する
- **`origin_selection_range` を返す理由。** 返さないとエディタが単語の切れ目で範囲を切り、`https` だけがリンクに見える。返すと下線と当たり判定が URL 全体になる

## 採らなかった案

| 案 | 採らなかった理由 |
| --- | --- |
| 説明欄の下に一覧（[0001](0001-links-in-the-description.md)） | 説明のどこにあった URL か分からず、パネルが縦に伸びる。想定されていた形でもなかった |
| `ShowDocumentHandler` を書いて開く | 要らなかった。`go_to_definition` が外部 scheme を自分で `open_url` に回す |
| 素のクリックで開く | カーソルを置く操作を奪う。編集中に踏みやすい |
| `Textarea` のまま、フォーカスの有無で描画を差し替える | 入力欄を出し入れすることになり、IME の変換中に差し替わる事故を持ち込む |

## 結果

**良くなったこと。** 説明を編集したまま URL がリンクに見え、`Cmd` / `Ctrl` + クリックで開く。0001 で足した一覧の行は不要になったので消した。

**引き受けた不都合。**

- アプリで一番使う入力欄が、コードエディタのモードで動くようになった。行番号も折りたたみも切ってあるが、**素性としてはコードエディタ**である
- そのため、**日本語 IME・`Tab` キー・字体・行の高さ**は、ヘッドレステストでは確かめられない。`docs/DESIGN.md` の完了条件 7・8 は実機での確認を求めており、この変更はそこに寄りかかっている
- `lsp-types` が直接の依存に増えた。`DefinitionProvider` の戻り値の型に要る

編集そのものが壊れていないことは、`view_tests::keeps_the_description_editable_with_links_in_it` が見ている（説明欄にフォーカスして打ち直し、保存が SQLite まで届くこと）。ただしこれは IME を通らない経路なので、**IME の確認を代替しない**。
