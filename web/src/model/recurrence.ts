// 繰り返しの判断（#198、[ADR 0049]）。
//
// **ここが繰り返しの唯一の実装です。** いつ次のカードを出すか、いつ前のカードを
// 片付けるか、毎月 31 日が 2 月にどこへ落ちるか——全部ここで決まります。Rust に
// あるのは形と行だけです（[ADR 0039]、[ADR 0040]）。
//
// **時計を読みません。** 「今日」は基準日（`"YYYY-MM-DD"`）として引数で受け、
// それを作るのは `web/src/state/day.ts` の 1 か所です（[ADR 0048]）。`parseDueDate`
// と同じ形にしてあるのは、境界の前後をテストで自由に作れるようにするためです。
//
// **発生日の判定は述語 1 つ**（[`matches`]）に集約し、そこから「その日以降で
// 最初」「その日以前で最後」を有限歩数の走査で出します。閉じた式で書くと、
// `weekday` の土日跳ばしと `monthly` の丸めがそれぞれ別の形で散り、境界を
// 確かめる場所が増えます。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
// [ADR 0040]: ../../../docs/adr/0040-the-shape-and-the-store-stay-in-rust.md
// [ADR 0048]: ../../../docs/adr/0048-the-day-turns-at-four-in-the-morning.md
// [ADR 0049]: ../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md

import type { Card } from "../ipc/types/Card";
import type { Recurrence } from "../ipc/types/Recurrence";
import type { Schedule } from "../ipc/types/Schedule";
import { addDays, formatIsoDate, fromParts, parseIsoDate, weekdayFromMonday } from "./dates";

/// 走査の上限（日）。**どの周期でも 1 年あれば必ず 1 回は当たります**——
/// いちばん間隔の空く `monthly` でも 31 日に 1 回です。当たらないまま抜けたら
/// それは周期のほうが壊れているので、`null` を返して何も起こしません。
const MAX_SCAN = 400;

/// その日が発生日か。**周期の意味はここに全部あります。**
///
/// `monthly` の日は**その月に無ければ月末に丸めます**（毎月 31 日の定義は、
/// 2 月には 28 日か 29 日に落ちます）。丸めをここに置くと、「次の発生日」も
/// 「前の発生日」も同じ規則で動きます。
export function matches(schedule: Schedule, date: string): boolean {
  const at = parseIsoDate(date);
  if (at === null) return false;
  switch (schedule.kind) {
    case "daily":
      return true;
    // 平日は月〜金。祝日はまだ見ません——`weekday` と `weekly{月〜金}` を
    // 分けてあるのは、見るようになったときの受け皿がここだからです。
    case "weekday":
      return weekdayFromMonday(at) <= 4;
    case "weekly":
      return schedule.days.includes(weekdayFromMonday(at));
    case "monthly":
      return at.getUTCDate() === Math.min(schedule.day, lastDayOfMonth(at));
    case "monthlyLast":
      return at.getUTCDate() === lastDayOfMonth(at);
  }
}

/// その月の末日。
function lastDayOfMonth(at: Date): number {
  return new Date(Date.UTC(at.getUTCFullYear(), at.getUTCMonth() + 1, 0)).getUTCDate();
}

/// `from` 日を `days` 日ずらした `"YYYY-MM-DD"`。読めない日付なら `null`。
export function shiftDay(day: string, days: number): string | null {
  const at = parseIsoDate(day);
  if (at === null) return null;
  return formatIsoDate(addDays(at, days));
}

/// `from`（含む）から先で、最初の発生日。見つからなければ `null`。
export function firstOccurrenceOnOrAfter(schedule: Schedule, from: string): string | null {
  return scan(schedule, from, 1);
}

/// `after`（含まない）より先で、最初の発生日。
export function firstOccurrenceAfter(schedule: Schedule, after: string): string | null {
  const next = shiftDay(after, 1);
  return next === null ? null : firstOccurrenceOnOrAfter(schedule, next);
}

/// `until`（含む）まで遡って、最後の発生日。見つからなければ `null`。
export function lastOccurrenceOnOrBefore(schedule: Schedule, until: string): string | null {
  return scan(schedule, until, -1);
}

/// 1 日ずつ歩いて、最初に当たった発生日を返す。
function scan(schedule: Schedule, from: string, step: number): string | null {
  let day: string | null = from;
  for (let walked = 0; walked < MAX_SCAN && day !== null; walked += 1) {
    if (matches(schedule, day)) return day;
    day = shiftDay(day, step);
  }
  return null;
}

/// 先読みの日数。**`daily` と `weekday` は 0 に固定**します（#198）。
///
/// 毎日出るものに期限を持たせないのと同じ理由で、先読みも持ちません。1 日の
/// うちに 2 枚が並ぶ形になり、どちらが今日のぶんか読めなくなります。
export function leadDaysOf(schedule: Schedule, leadDays: number): number {
  if (schedule.kind === "daily" || schedule.kind === "weekday") return 0;
  return Math.max(0, Math.trunc(leadDays));
}

/// 出るカードの期限。**`daily` と `weekday` は期限なし**（#198）。
///
/// 毎日のものに期限を付けると、`⚠` が毎日 1 件ずつ増えます。「今日やること」
/// であることは、そこに出ていること自体が言っています。
export function dueDateOf(schedule: Schedule, occurrence: string): string | null {
  if (schedule.kind === "daily" || schedule.kind === "weekday") return null;
  return occurrence;
}

/// いま出すべき発生日。無ければ `null`（#198）。
///
/// - **まだ 1 枚も出していない定義**（`lastGeneratedOn` が空）は、基準日以降で
///   **最初**の発生日を狙います。過去の発生日まで遡って出すと、定義を作った
///   その日に「先月ぶん」が出ます
/// - **一度でも出している定義**は、`lastGeneratedOn` より後で、先読みの窓が
///   開いているもののうち**最後**の 1 つだけを出します。1 ヶ月ぶりに開いても
///   30 枚生まれないのはこれで、切り捨てたことは画面に出しません
export function occurrenceToGenerate(recurrence: Recurrence, today: string): string | null {
  if (!recurrence.enabled) return null;
  const { schedule } = recurrence;
  const lead = leadDaysOf(schedule, recurrence.leadDays);
  const last = recurrence.lastGeneratedOn;

  if (last === null) {
    const first = firstOccurrenceOnOrAfter(schedule, today);
    if (first === null) return null;
    return windowIsOpen(first, lead, today) ? first : null;
  }

  // 先読みの窓が開いている、いちばん先の発生日。ここから遡って探せば、
  // 取りこぼしのうち最新の 1 回が最初に当たります。
  const limit = shiftDay(today, lead);
  if (limit === null) return null;
  const latest = lastOccurrenceOnOrBefore(schedule, limit);
  if (latest === null || latest <= last) return null;
  return latest;
}

/// 発生日 `occurrence` の先読みの窓が、基準日で開いているか。
function windowIsOpen(occurrence: string, lead: number, today: string): boolean {
  const opens = shiftDay(occurrence, -lead);
  return opens !== null && opens <= today;
}

/// そのカードを片付ける日が来たか（#198）。
///
/// **数えるのは生成日ではなく、そのカードの次の発生日**です。先読みのある定義で
/// 生成と同時に片付けると、まだ手を動かしているカードが消えます——月次で
/// 先読み 7 日なら、前月 25 日に次のカードが出て、当月 1 日に前のカードが
/// 片付きます。
export function shouldCleanUp(schedule: Schedule, occurrence: string, today: string): boolean {
  const next = firstOccurrenceAfter(schedule, occurrence);
  return next !== null && next <= today;
}

/// 片付ける相手。**盤面の上のカードだけ**を見ます。
///
/// アーカイブに入っているものは、もう片付いています。`previous` が `keep` の
/// 定義は誰も片付けないので、はじめから外します。
export function cardsToCleanUp(
  recurrence: Recurrence,
  cards: readonly Card[],
  today: string,
): Card[] {
  if (recurrence.previous === "keep") return [];
  return cards.filter(
    (card) =>
      card.recurrenceId === recurrence.id &&
      card.occurrenceDate !== null &&
      shouldCleanUp(recurrence.schedule, card.occurrenceDate, today),
  );
}

/// 出来たカードの入れ先。**定義のカラムが消えていたら一番左**（#198）。
///
/// 入れ先が無くなったからといって生成を止めません。止めると、カラムを 1 本
/// 消した日から繰り返しが黙って動かなくなります。
export function targetColumnId(
  recurrence: Recurrence,
  columns: readonly { id: number }[],
): number | null {
  if (columns.some((column) => column.id === recurrence.columnId)) return recurrence.columnId;
  return columns[0]?.id ?? null;
}

/// 周期を日本語で 1 行に。パネルと、繰り返しの説明に使います。
export function describeSchedule(schedule: Schedule): string {
  const names = ["月", "火", "水", "木", "金", "土", "日"];
  switch (schedule.kind) {
    case "daily":
      return "毎日";
    case "weekday":
      return "平日";
    case "weekly": {
      const days = [...schedule.days]
        .sort((left, right) => left - right)
        .map((day) => names[day] ?? "?")
        .join("・");
      return days === "" ? "毎週" : `毎週 ${days}`;
    }
    case "monthly":
      return `毎月 ${String(schedule.day)} 日`;
    case "monthlyLast":
      return "毎月末";
  }
}

/// 「2026-02-31」のような無い日付を弾いて、`"YYYY-MM-DD"` に正す。
///
/// パネルが打たれた値を確かめるのに使います。**モデルの側に置く**のは、
/// 同じ規則を画面ごとに書き直さないためです。
export function normalizeDay(year: number, month: number, day: number): string | null {
  const at = fromParts(year, month, day);
  return at === null ? null : formatIsoDate(at);
}
