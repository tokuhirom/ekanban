// カードの右クリックメニューから当てる期限の候補（#132）。
//
// **今日が何日かは Rust から来ます**（`Snapshot.today`）。ブラウザの時計から
// 決めると、`due_statuses` を出した日とここが数える日が食い違います
// （`docs/DESIGN.md`「絞り込みと検索」）。

export interface DueChoice {
  label: string;
  /** `set_card_due_date` に渡す形。`""` は期限なし。 */
  date: string;
}

/// 右クリックメニューに出す期限の候補。
///
/// 週の起点は月曜です。「来週」は次に来る月曜。読めない `today` が来たら
/// 候補を出しません——当てずっぽうの日付を入れるより、項目が無いほうが安全です。
export function dueChoices(today: string): DueChoice[] {
  const base = parseIsoDate(today);
  if (base === null) return [];
  // 月曜を 0 とした曜日。`getUTCDay()` は日曜が 0 なので 1 つずらす。
  const fromMonday = (base.getUTCDay() + 6) % 7;
  return [
    { label: "今日", date: formatIsoDate(base) },
    { label: "明日", date: formatIsoDate(addDays(base, 1)) },
    { label: "来週", date: formatIsoDate(addDays(base, 7 - fromMonday)) },
  ];
}

/// `"YYYY-MM-DD"` を UTC の 0 時として読む。
///
/// **UTC で持ちます。** 地方時で作ると、`Date` の日付が動かしている機械の
/// タイムゾーンで 1 日ずれます。
function parseIsoDate(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (match === null) return null;
  const [, year = "", month = "", day = ""] = match;
  const date = new Date(Date.UTC(Number(year), Number(month) - 1, Number(day)));
  return Number.isNaN(date.getTime()) ? null : date;
}

function addDays(date: Date, days: number): Date {
  return new Date(date.getTime() + days * 86_400_000);
}

function formatIsoDate(date: Date): string {
  return date.toISOString().slice(0, 10);
}
