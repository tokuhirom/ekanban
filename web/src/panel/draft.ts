// カードの編集パネルが持つ下書きと、その上の純粋な操作。
//
// **下書きは webview のものです**（`docs/TAURI-MIGRATION.md` §2）。打っている
// 途中の値を Rust に渡さないので、確定するまでここに溜まります。確定は
// `update_card` の 1 回で、チェックリストも項目ごと一括で渡します（§3）。
//
// ここに盤面の論理は入りません。入るのは「配列の何番目を入れ替える」までで、
// 期限の書式が正しいかどうかのような判定は Rust に残します（§5）——同じ判定を
// TypeScript にもう 1 つ持つと、2 つがずれた日に画面と保存が食い違います。

import type { Card } from "../ipc/types/Card";
import type { ChecklistItemDraft } from "../ipc/types/ChecklistItemDraft";

export interface CardDraft {
  title: string;
  description: string;
  /** `"YYYY-MM-DD"` か空文字。読めるかどうかを決めるのは Rust。 */
  dueDate: string;
  tagIds: number[];
  checklist: ChecklistItemDraft[];
}

/// 保存済みのカードから下書きを起こす。
export function draftOf(card: Card): CardDraft {
  return {
    title: card.title,
    description: card.description,
    dueDate: card.dueDate ?? "",
    tagIds: [...card.tagIds],
    checklist: card.checklistItems.map((item) => ({
      id: item.id,
      text: item.text,
      checked: item.checked,
    })),
  };
}

/// 新しいカードの下書き。
///
/// **どの欄にも案内の文言を入れません**（`docs/DESIGN.md`）。既定値として
/// 入れると、消し忘れがそのままデータになります。案内は placeholder が出します。
export function emptyDraft(): CardDraft {
  return { title: "", description: "", dueDate: "", tagIds: [], checklist: [] };
}

/// 保存できる状態か。
///
/// 見ているのは**空白かどうかだけ**です。期限の書式は Rust が読んで
/// `Validation` で返すので、ここでは判定しません。押せるのに断る操作は理由を
/// 言わずにコントロールを無効にする、が `docs/DESIGN.md` の規則なので、
/// 空白のタイトルでは保存ボタンを押せなくします。
export function draftIsSavable(draft: CardDraft): boolean {
  return (
    draft.title.trim() !== "" && !draft.checklist.some((item) => item.text.trim() === "")
  );
}

export function toggleTag(tagIds: readonly number[], tagId: number): number[] {
  return tagIds.includes(tagId)
    ? tagIds.filter((id) => id !== tagId)
    : [...tagIds, tagId];
}

export function setChecklistText(
  checklist: readonly ChecklistItemDraft[],
  index: number,
  text: string,
): ChecklistItemDraft[] {
  return checklist.map((item, at) => (at === index ? { ...item, text } : item));
}

export function toggleChecklistItem(
  checklist: readonly ChecklistItemDraft[],
  index: number,
): ChecklistItemDraft[] {
  return checklist.map((item, at) => (at === index ? { ...item, checked: !item.checked } : item));
}

export function deleteChecklistItem(
  checklist: readonly ChecklistItemDraft[],
  index: number,
): ChecklistItemDraft[] {
  return checklist.filter((_, at) => at !== index);
}

/// 項目を 1 つぶん上げ下げする。端では動かさない。
///
/// `id` は保存済みの項目の ID で、`null` はまだ保存していない項目です。並び順
/// そのものは配列の順で `update_card` に渡すので、`position` はここで触りません。
export function moveChecklistItem(
  checklist: readonly ChecklistItemDraft[],
  index: number,
  direction: "up" | "down",
): ChecklistItemDraft[] {
  const to = direction === "up" ? index - 1 : index + 1;
  if (index < 0 || index >= checklist.length || to < 0 || to >= checklist.length) {
    return [...checklist];
  }
  const moved = [...checklist];
  const [item] = moved.splice(index, 1);
  if (item === undefined) return [...checklist];
  moved.splice(to, 0, item);
  return moved;
}

/// 期限の近道が入れる日付。
///
/// **今日が何日かは Rust から来ます**（`Snapshot.today`）。ブラウザの時計から
/// 決めると、`due_statuses` を出した日と近道が入れる日が食い違います。
///
/// 週の起点は月曜です（gpui 版の `num_days_from_monday` と同じ）。「今週末」は
/// 次に来る土曜で、今日が土曜ならその日。「来週」は次の月曜。
export function quickDueDates(today: string): { label: string; date: string }[] {
  const base = parseIsoDate(today);
  if (base === null) return [];
  // 月曜を 0 とした曜日。`getUTCDay()` は日曜が 0 なので 1 つずらす。
  const fromMonday = (base.getUTCDay() + 6) % 7;
  return [
    { label: "今日", date: formatIsoDate(base) },
    { label: "明日", date: formatIsoDate(addDays(base, 1)) },
    { label: "今週末", date: formatIsoDate(addDays(base, (5 - fromMonday + 7) % 7)) },
    { label: "来週", date: formatIsoDate(addDays(base, 7 - fromMonday)) },
  ];
}

/// `"YYYY-MM-DD"` を UTC の正午として読む。
///
/// **UTC で持ちます。** 地方時で作ると、`Date` の日付が実行している機械の
/// タイムゾーンで 1 日ずれます（§3 が期限を文字列で運んでいるのと同じ理由）。
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
