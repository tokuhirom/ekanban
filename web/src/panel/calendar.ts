// 期限の欄に出すカレンダーの升目（#194、[ADR 0046]）。
//
// **日付の足し算はしません。** `model/dates.ts` に任せます——期限の読み方
// （`model/due.ts`）と同じ土台から数えないと、カレンダーが指す日と、打った文字が
// 読まれる日が食い違います（`docs/DESIGN.md`）。
//
// 月は `"YYYY-MM"` の文字列で持ちます。日付と同じく文字列で持つのは、月送りの
// たびに `Date` を持ち回ると、地方時で作られたものが 1 つ混ざるだけで日付が
// 1 日ずれるからです。
//
// [ADR 0046]: ../../../docs/adr/0046-picking-a-due-date-from-a-calendar.md

import { addDays, formatIsoDate, fromParts, parseIsoDate, weekdayFromMonday } from "../model/dates";

/** 升目の見出し。月曜から。`toLocaleDateString` に任せない——ロケールで言葉が変わる。 */
export const CALENDAR_WEEKDAYS = ["月", "火", "水", "木", "金", "土", "日"] as const;

/// 升目に出す 1 日。
export interface CalendarDay {
  /** `"YYYY-MM-DD"`。押されたらこれがそのまま欄に入る。 */
  date: string;
  /** 日にちの数字。見出しに出す。 */
  day: number;
  /** 出している月の日か。前後の月からはみ出した日は `false`。 */
  inMonth: boolean;
}

/// `"YYYY-MM-DD"` が属する月（`"YYYY-MM"`）。読めなければ `null`。
export function monthOf(date: string): string | null {
  return parseIsoDate(date) === null ? null : date.slice(0, 7);
}

/// 月を送る。`delta` は月の数で、年をまたぐ。作れない月（0 年より前）は `null`。
export function shiftMonth(month: string, delta: number): string | null {
  const parts = parseMonth(month);
  if (parts === null) return null;
  // 0 起点に均してから足すと、12 月から 1 月への繰り上がりを自分で書かずに済む。
  const total = parts.year * 12 + (parts.month - 1) + delta;
  if (total < 0) return null;
  const year = Math.floor(total / 12);
  const month1 = (total % 12) + 1;
  return fromParts(year, month1, 1) === null ? null : formatMonth(year, month1);
}

/// 月の見出し（`2026年9月`）。読めない月では空文字。
///
/// `toLocaleDateString` に任せないのは、曜日の名前と同じ理由です（`model/due.ts`）。
export function monthLabel(month: string): string {
  const parts = parseMonth(month);
  return parts === null ? "" : `${String(parts.year)}年${String(parts.month)}月`;
}

/// 升目の 1 日を読み上げるための言葉（`2026年9月12日`）。読めない日付では空文字。
///
/// 升目に出ているのは日にちの数字だけなので、押せるものの名前をここで作ります。
/// 曜日と同じく、言葉はこちらで持ちます——`toLocaleDateString` に任せると
/// webview のロケール次第で変わります。
export function dayLabel(date: string): string {
  const parsed = parseIsoDate(date);
  if (parsed === null) return "";
  return `${monthLabel(date.slice(0, 7))}${String(parsed.getUTCDate())}日`;
}

/// 月の升目。**常に 6 週ぶん返します。**
///
/// 月によって週の数が変わると、月を送るたびにカレンダーの高さが変わり、下に
/// ある欄がその都度動きます。前後の月からはみ出した日は `inMonth: false` で、
/// 押せば選べます——月を送らずに月末月初へ渡れます。
///
/// 読めない月と、`"YYYY-MM-DD"` で書けない日を含む月では空を返します。
/// 当てずっぽうの日付を升目に出すより、出さないほうが安全です。
export function monthWeeks(month: string): CalendarDay[][] {
  const parts = parseMonth(month);
  if (parts === null) return [];
  const first = fromParts(parts.year, parts.month, 1);
  if (first === null) return [];
  // 升目は月曜始まり。月の 1 日が何曜日かのぶんだけ、前の月から借りる。
  const begin = addDays(first, -weekdayFromMonday(first));

  const weeks: CalendarDay[][] = [];
  for (let week = 0; week < 6; week += 1) {
    const days: CalendarDay[] = [];
    for (let weekday = 0; weekday < 7; weekday += 1) {
      const date = addDays(begin, week * 7 + weekday);
      const iso = formatIsoDate(date);
      if (iso === null) return [];
      days.push({
        date: iso,
        day: date.getUTCDate(),
        inMonth: date.getUTCMonth() === parts.month - 1,
      });
    }
    weeks.push(days);
  }
  return weeks;
}

function parseMonth(month: string): { year: number; month: number } | null {
  const match = /^(\d{4})-(\d{2})$/.exec(month);
  if (match === null) return null;
  const [, year = "", value = ""] = match;
  // 13 月のような月は `fromParts` が弾く。
  return fromParts(Number(year), Number(value), 1) === null
    ? null
    : { year: Number(year), month: Number(value) };
}

function formatMonth(year: number, month: number): string {
  return `${String(year).padStart(4, "0")}-${String(month).padStart(2, "0")}`;
}
