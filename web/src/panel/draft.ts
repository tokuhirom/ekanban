// カードの編集パネルが持つ下書きと、その上の純粋な操作。
//
// **下書きは webview のものです**（`docs/DESIGN.md`「状態の持ち主」）。打っている
// 途中の値を Rust に渡さないので、確定するまでここに溜まります。確定は
// `update_card` の 1 回で、チェックリストも項目ごと一括で渡します（`docs/DESIGN.md`「コマンドとイベント」）。
//
// ここに盤面の論理は入りません。入るのは「配列の何番目を入れ替える」までで、
// 期限の書式が正しいかどうかのような判定は Rust に残します（`docs/DESIGN.md`「絞り込みと検索」）——同じ判定を
// TypeScript にもう 1 つ持つと、2 つがずれた日に画面と保存が食い違います。

import type { Card } from "../ipc/types/Card";
import type { ChecklistItemDraft } from "../ipc/types/ChecklistItemDraft";

/// 下書きの中だけのチェックリスト項目。
///
/// `ChecklistItemDraft`（`ts-rs` が Rust から起こす、IPC に載る形）に `key` を
/// 足したものです。**`key` は画面の鍵で、Rust には渡りません**——`@dnd-kit` は
/// 並べ替えのあいだ変わらない `id` を要るのに、まだ保存していない項目は `id` を
/// 持たないためです。添字を鍵にすると、並べ替えた瞬間に鍵が入れ替わります。
export interface DraftChecklistItem {
  key: string;
  id: number | null;
  text: string;
  checked: boolean;
}

export interface CardDraft {
  title: string;
  description: string;
  /** `"YYYY-MM-DD"` か空文字。読めるかどうかを決めるのは Rust。 */
  dueDate: string;
  tagIds: number[];
  checklist: DraftChecklistItem[];
}

/// 下書きの鍵を採番する。`key` は下書きの中でしか使わないので、通し番号で足りる。
let nextChecklistKey = 0;

/// まだ保存していない項目を 1 つ作る。
///
/// `id` は `null` で、`update_card` がこれを「新しい項目」として読みます。
export function newChecklistItem(): DraftChecklistItem {
  nextChecklistKey += 1;
  return { key: `new-${nextChecklistKey}`, id: null, text: "", checked: false };
}

/// Rust に渡す形にする。**`key` はここで落とします。**
export function checklistToSend(
  checklist: readonly DraftChecklistItem[],
): ChecklistItemDraft[] {
  return checklist.map((item) => ({ id: item.id, text: item.text, checked: item.checked }));
}

/// 保存済みのカードから下書きを起こす。
export function draftOf(card: Card): CardDraft {
  return {
    title: card.title,
    description: card.description,
    dueDate: card.dueDate ?? "",
    tagIds: [...card.tagIds],
    checklist: card.checklistItems.map((item) => ({
      key: `saved-${item.id}`,
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
/// 見ているのは**タイトルが空白かどうかだけ**です。期限の書式は Rust が読んで
/// `Validation` で返すので、ここでは判定しません。押せるのに断る操作は理由を
/// 言わずにコントロールを無効にする、が `docs/DESIGN.md` の規則なので、
/// 空白のタイトルでは保存ボタンを押せなくします。
///
/// **名前の入っていないチェックリスト項目は止めません**（#114）。保存のときに
/// Rust が落とします。判定をこちらにもう 1 つ置くと、2 つがずれた日に画面と
/// 保存が食い違います。
export function draftIsSavable(draft: CardDraft): boolean {
  return draft.title.trim() !== "";
}

export function toggleTag(tagIds: readonly number[], tagId: number): number[] {
  return tagIds.includes(tagId)
    ? tagIds.filter((id) => id !== tagId)
    : [...tagIds, tagId];
}

export function setChecklistText(
  checklist: readonly DraftChecklistItem[],
  index: number,
  text: string,
): DraftChecklistItem[] {
  return checklist.map((item, at) => (at === index ? { ...item, text } : item));
}

export function toggleChecklistItem(
  checklist: readonly DraftChecklistItem[],
  index: number,
): DraftChecklistItem[] {
  return checklist.map((item, at) => (at === index ? { ...item, checked: !item.checked } : item));
}

export function deleteChecklistItem(
  checklist: readonly DraftChecklistItem[],
  index: number,
): DraftChecklistItem[] {
  return checklist.filter((_, at) => at !== index);
}

/// 項目を 1 つぶん上げ下げする。端では動かさない。
///
/// `id` は保存済みの項目の ID で、`null` はまだ保存していない項目です。並び順
/// そのものは配列の順で `update_card` に渡すので、`position` はここで触りません。
export function moveChecklistItem(
  checklist: readonly DraftChecklistItem[],
  index: number,
  direction: "up" | "down",
): DraftChecklistItem[] {
  const to = direction === "up" ? index - 1 : index + 1;
  return moveTo(checklist, index, to);
}

/// 掴んだ項目を、落とし先の項目のいた位置へ入れる（#113）。
///
/// **何番目に落ちたかを決めるのはこちらのコードです**（`docs/DESIGN.md`
/// 「ドラッグ＆ドロップ」）。`@dnd-kit` に任せるのは掴む・追う・落とすまで。
/// 鍵が見つからない、または落とし先が掴んだものと同じなら、並びは変わりません。
export function reorderChecklist(
  checklist: readonly DraftChecklistItem[],
  fromKey: string,
  toKey: string,
): DraftChecklistItem[] {
  const from = checklist.findIndex((item) => item.key === fromKey);
  const to = checklist.findIndex((item) => item.key === toKey);
  if (from === -1 || to === -1) return [...checklist];
  return moveTo(checklist, from, to);
}

/// 配列の `index` 番目を `to` 番目へ移す。端の外なら動かさない。
function moveTo(
  checklist: readonly DraftChecklistItem[],
  index: number,
  to: number,
): DraftChecklistItem[] {
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
/// タイムゾーンで 1 日ずれます（`docs/DESIGN.md`「コマンドとイベント」が期限を文字列で運んでいるのと同じ理由）。
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
