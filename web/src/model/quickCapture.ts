// クイックキャプチャの 1 行の読み方（#205、[ADR 0051]）。
//
// 1 行を**タイトル・期限・タグ**に切り分けます。
//
// ```
// <タイトル> [@期限] [#タグ]…     ← 記号は行末側にまとめる。@ と # の順は問わない
// ```
//
// **行末から剥がします。** 残っている行の最後の `@` `＠` `#` `＃` を探し、そこから
// 行末までを 1 つのトークンとして読み、剥がせたら同じことを繰り返します。剥がせ
// なくなったところで止め、残りがタイトルです。
//
// | 打つもの | タイトル | 期限 | タグ |
// | --- | --- | --- | --- |
// | `牛乳を買う @today` | 牛乳を買う | 今日 | |
// | `棚卸し @+3` | 棚卸し | 3 日後 | |
// | `買い物@明日 #家事` | 買い物 | 明日 | 家事 |
// | `メールは foo@example.com` | （行全体） | — | — |
// | `打ち合わせ @明日 と資料` | （行全体） | — | — |
// | `#12 の件を確認 @金` | `#12 の件を確認` | 金 | — |
//
// 下の 3 行がこの規則の勘所です。**トークンに空白を含めない**ので
// `@明日 と資料` は触らず、**行末からしか剥がさない**ので先頭の `#12` は
// カード番号の見た目のまま残ります。読めないものには触らず、行全体がタイトルに
// なります——打っている途中の文字をまちがい扱いしないという [ADR 0031] の姿勢と
// 同じです。
//
// **期限の読み方はここに持ちません。** `@` のうしろは `model/due.ts` の
// `parseDueDate` にそのまま渡すので、`9/12`・`明日`・`金`・`+3` は欄に打つときと
// 同じに読まれます。
//
// [ADR 0031]: ../../../docs/adr/0031-typing-a-due-date.md
// [ADR 0051]: ../../../docs/adr/0051-typing-a-due-date-and-tags-in-quick-capture.md

import { parseDueDate } from "./due";

/// 1 行を切り分けた結果。
export interface QuickCaptureRead {
  /** 記号のトークンを外した残り。空になることもある（`@today` だけを打ったとき）。 */
  title: string;
  /** 読めた期限。`"YYYY-MM-DD"`。無ければ `null`。 */
  dueDate: string | null;
  /** 打たれた順のタグ名。ID を引くのは画面（`findTagByName`、無ければ作る）。 */
  tagNames: string[];
}

/** 期限を表す記号。全角も受ける——日本語入力のまま打てる。 */
const DUE_MARKS = "@＠";
/** タグを表す記号。 */
const TAG_MARKS = "#＃";

/// 1 行を、タイトル・期限・タグに切り分ける。`today` は `"YYYY-MM-DD"` の基準日。
export function readQuickCapture(line: string, today: string): QuickCaptureRead {
  let rest = line;
  let dueDate: string | null = null;
  const tagNames: string[] = [];

  for (;;) {
    // **毎回うしろの空白を落とします。** 落とさないと、`@明日 #家事` の `#家事`
    // を剥がしたあとに残る `@明日 ` が「空白を含むトークン」になってしまいます。
    rest = rest.trimEnd();
    const at = lastMarkIndex(rest);
    if (at === null) break;

    const token = rest.slice(at + 1);
    // 空白を含むトークンは剥がさない。`@明日 と資料` はここで止まる。
    if (token === "" || /\s/u.test(token)) break;

    if (DUE_MARKS.includes(rest[at] ?? "")) {
      // 期限は最後の 1 つだけ。2 つ目が来たらタイトルに残す。
      if (dueDate !== null) break;
      const read = parseDueDate(token, today);
      if (!read.ok || read.date === null) break;
      dueDate = read.date;
    } else {
      // 打たれた順に並べる。剥がすのは行末からなので、先頭に足していく。
      tagNames.unshift(token);
    }
    rest = rest.slice(0, at);
  }

  return { title: rest.trim(), dueDate, tagNames };
}

/// 最後の記号の位置。無ければ `null`。
function lastMarkIndex(text: string): number | null {
  for (let index = text.length - 1; index >= 0; index -= 1) {
    const character = text[index] ?? "";
    if (DUE_MARKS.includes(character) || TAG_MARKS.includes(character)) return index;
  }
  return null;
}
