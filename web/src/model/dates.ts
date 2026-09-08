// 日付そのものの扱い。**盤面の判断のうち、日付を数えるところの土台**です。
//
// 期限は日付だけを持ち、境界を越える形は `"YYYY-MM-DD"` の文字列です
// （`docs/DESIGN.md`「境界を越える値」）。ここもその形で受けて、その形で返します。
//
// **`Date` は UTC の 0 時としてしか使いません。** 地方時で作ると、動かしている
// 機械のタイムゾーンで日付が 1 日ずれます。日付の足し算をするためだけに `Date`
// を通し、外に出るときは必ず文字列に戻します。

/** 1 日のミリ秒。`Date` の足し算に使う。 */
const DAY = 86_400_000;

/// `"YYYY-MM-DD"` を UTC の 0 時として読む。読めなければ `null`。
///
/// **月末を越える日付を受け取りません。** `Date.UTC` は 2 月 30 日を 3 月 2 日に
/// 繰り上げるので、読めたことにすると打った日付と保存される日付が食い違います。
/// 組み立て直したものと突き合わせて確かめます。
export function parseIsoDate(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (match === null) return null;
  const [, year = "", month = "", day = ""] = match;
  return fromParts(Number(year), Number(month), Number(day));
}

/// 年・月・日から作る。無い日付（2 月 30 日、13 月）なら `null`。
export function fromParts(year: number, month: number, day: number): Date | null {
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const date = new Date(Date.UTC(year, month - 1, day));
  if (Number.isNaN(date.getTime())) return null;
  // 繰り上がっていたら、その日付は無い。
  if (
    date.getUTCFullYear() !== year ||
    date.getUTCMonth() !== month - 1 ||
    date.getUTCDate() !== day
  ) {
    return null;
  }
  return date;
}

/// `"YYYY-MM-DD"` にする。4 桁で書けない年は `null`。
///
/// 4 桁に収まらない年を弾くのは、`"YYYY-MM-DD"` という約束のほうを守るためです。
/// `+275760-09-13` のような文字列は、保存しても読み戻せません。
export function formatIsoDate(date: Date): string | null {
  const year = date.getUTCFullYear();
  if (Number.isNaN(year) || year < 0 || year > 9999) return null;
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  return `${String(year).padStart(4, "0")}-${month}-${day}`;
}

export function addDays(date: Date, days: number): Date {
  return new Date(date.getTime() + days * DAY);
}

/// 月曜を 0 とした曜日。`getUTCDay()` は日曜が 0 なので 1 つずらす。
export function weekdayFromMonday(date: Date): number {
  return (date.getUTCDay() + 6) % 7;
}

/// 2 つの日付の間の日数（`to - from`）。
export function daysBetween(from: Date, to: Date): number {
  return Math.round((to.getTime() - from.getTime()) / DAY);
}
