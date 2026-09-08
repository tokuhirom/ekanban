// 期限として打たれた文字の読み方（#134、[ADR 0031]）。
//
// **基準日は引数で受けます。** ここで時計を読むと、期限の状態を出した日と
// 「明日」が数えた日が食い違います（`docs/DESIGN.md`「絞り込みと検索」）。
//
// 受ける形は次のとおりです。全角で打たれたものは、検索と同じ
// `normalizeSearchText` で半角・小文字に均してから読みます。
//
// | 打つもの | 読み |
// | --- | --- |
// | `2026-09-12` | そのまま |
// | `9/12`、`9-12` | 今年。過ぎていれば来年 |
// | `今日` `明日` `明後日` / `today` `tomorrow` | そのまま |
// | `月`（`月曜`・`月曜日`）/ `mon` `monday` … | 次に来るその曜日。今日が同じ曜日なら 7 日後 |
// | `+3` | 3 日後 |
// | `来週` | 次の月曜 |
// | `今週末` | 今日を含めて次に来る土曜 |
//
// **`今週末` だけ今日を含めます。** 今日が土曜なら今日が「今週末」です。曜日
// そのものを打ったときは今日を含めません——`土` と打つ人は今日のことを
// 言っていないためです。
//
// [ADR 0031]: ../../../docs/adr/0031-typing-a-due-date.md

import { addDays, formatIsoDate, fromParts, parseIsoDate, weekdayFromMonday } from "./dates";
import { normalizeSearchText } from "./search";

/// 打たれた文字を読んだ結果。
///
/// 読めなかったことを例外にしないのは、それが入力欄の脇に出すもので、
/// 打っている途中はふつうに起きるからです（`docs/DESIGN.md`「アプリが伝えること」）。
export type DueDateRead =
  /** 読めた。`null` は「期限なし」（空欄）。 */
  | { ok: true; date: string | null }
  /** 読めなかった。打たれた文字をそのまま返す（欄の脇に出すため）。 */
  | { ok: false; typed: string };

/// 読めなかったときに、入力欄の脇に出す言葉。
///
/// **何が打てるかをそのまま言います。** 「日付として正しくありません」だけでは、
/// 次に何を打てばよいかが分かりません（`docs/DESIGN.md`「アプリが伝えること」）。
export const DUE_DATE_HELP =
  "日付として読めません。9/12、明日、金、+3 のように入力してください（空欄で期限なし）";

/** 曜日の名前。月曜から。`toLocaleDateString` に任せない——ロケールで言葉が変わる。 */
const WEEKDAYS = ["月", "火", "水", "木", "金", "土", "日"] as const;

const NEARBY_DAYS: Record<string, number> = {
  今日: 0,
  きょう: 0,
  today: 0,
  明日: 1,
  あした: 1,
  tomorrow: 1,
  明後日: 2,
  あさって: 2,
};

const WEEKDAY_NAMES: [string, string, string][] = [
  ["月", "mon", "monday"],
  ["火", "tue", "tuesday"],
  ["水", "wed", "wednesday"],
  ["木", "thu", "thursday"],
  ["金", "fri", "friday"],
  ["土", "sat", "saturday"],
  ["日", "sun", "sunday"],
];

/// 期限として打たれた文字を読む。`today` は `"YYYY-MM-DD"` の基準日。
export function parseDueDate(value: string, today: string): DueDateRead {
  const typed = value.trim();
  if (typed === "") return { ok: true, date: null };
  const base = parseIsoDate(today);
  if (base === null) return { ok: false, typed };
  const read = readDueDate(normalizeSearchText(typed).trim(), base);
  const date = read === null ? null : formatIsoDate(read);
  return date === null ? { ok: false, typed } : { ok: true, date };
}

/// 打った文字をどう読んだか。確定する前に欄の下に出す（[ADR 0031]）。
///
/// 空欄と読めない文字では何も出しません——出す言葉がありません。
///
/// [ADR 0031]: ../../../docs/adr/0031-typing-a-due-date.md
export function dueDatePreview(
  value: string,
  today: string,
): { date: string; label: string } | null {
  const read = parseDueDate(value, today);
  if (!read.ok || read.date === null) return null;
  const date = parseIsoDate(read.date);
  if (date === null) return null;
  return { date: read.date, label: `${read.date}（${WEEKDAYS[weekdayFromMonday(date)]}）` };
}

/// 均したあとの文字を日付にする。読めなければ `null`。
function readDueDate(text: string, today: Date): Date | null {
  const fromMonday = weekdayFromMonday(today);

  const nearby = NEARBY_DAYS[text];
  if (nearby !== undefined) return addDays(today, nearby);
  // 次の月曜。今日が月曜なら 7 日後で、「来週」が今日にならない。
  if (text === "来週") return addDays(today, 7 - fromMonday);
  // 今日を含めて次に来る土曜。
  if (text === "今週末") return addDays(today, (5 - fromMonday + 7) % 7);

  if (text.startsWith("+")) {
    const days = text.slice(1);
    if (!/^[+-]?\d+$/.test(days)) return null;
    return addDays(today, Number(days));
  }

  const weekday = weekdayIndex(text);
  if (weekday !== null) {
    // 「次に来る」ほうを採る。今日が同じ曜日なら 7 日後。
    const ahead = (weekday - fromMonday + 7) % 7;
    return addDays(today, ahead === 0 ? 7 : ahead);
  }

  const iso = parseIsoDate(text);
  if (iso !== null) return iso;

  return readMonthAndDay(text, today);
}

/// 曜日を表す語を、月曜を 0 とした番号にする。
function weekdayIndex(text: string): number | null {
  // 「月曜日」「月曜」「月」のどれでも同じ。英語は 3 文字と綴り全体を受ける。
  const trimmed = text.endsWith("曜日")
    ? text.slice(0, -2)
    : text.endsWith("曜")
      ? text.slice(0, -1)
      : text;
  const index = WEEKDAY_NAMES.findIndex((names) => names.includes(trimmed));
  return index === -1 ? null : index;
}

/// `9/12` と `9-12`。年は今年で、今日より前になるなら来年。
function readMonthAndDay(text: string, today: Date): Date | null {
  const separator = text.includes("/") ? "/" : "-";
  const at = text.indexOf(separator);
  if (at <= 0 || at === text.length - 1) return null;
  const month = text.slice(0, at);
  const day = text.slice(at + 1);
  if (!/^\d+$/.test(month) || !/^\d+$/.test(day)) return null;

  const thisYear = fromParts(today.getUTCFullYear(), Number(month), Number(day));
  if (thisYear === null) return null;
  if (thisYear.getTime() >= today.getTime()) return thisYear;
  // 2/29 は来年に無いことがある。無ければ読めなかったことにする。
  return fromParts(today.getUTCFullYear() + 1, Number(month), Number(day));
}
