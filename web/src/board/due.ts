// カードの右クリックメニューから当てる期限の候補（#132）。
//
// **今日が何日かは呼ぶ側から来ます**（`BoardState.today`）。ここで時計を読むと、
// 期限の状態を出した日とここが数える日が食い違います（`docs/DESIGN.md`
// 「絞り込みと検索」）。日付の足し算は `model/dates.ts` の 1 か所に置きます。

import { addDays, formatIsoDate, parseIsoDate, weekdayFromMonday } from "../model/dates";

export interface DueChoice {
  label: string;
  /** `setCardDueDate` に渡す形（`"YYYY-MM-DD"`）。 */
  date: string;
}

/// 右クリックメニューに出す期限の候補。
///
/// 週の起点は月曜です。「来週」は次に来る月曜。読めない `today` が来たら
/// 候補を出しません——当てずっぽうの日付を入れるより、項目が無いほうが安全です。
export function dueChoices(today: string): DueChoice[] {
  const base = parseIsoDate(today);
  if (base === null) return [];
  const fromMonday = weekdayFromMonday(base);
  return [
    { label: "今日", date: base },
    { label: "明日", date: addDays(base, 1) },
    { label: "来週", date: addDays(base, 7 - fromMonday) },
  ]
    .map((choice) => ({ label: choice.label, date: formatIsoDate(choice.date) }))
    .filter((choice): choice is DueChoice => choice.date !== null);
}
