// アーカイブ表示の並べ方（[ADR 0010]）。
//
// **アーカイブでは、絞り込みから外れたカードを隠します。** 盤面では減光する
// （落とす位置が動かないように）のに対し、ここには落とす位置がありません。
//
// 日ごとにまとめ、新しい日から並べます。`archivedAt` はエポックからのミリ秒
// なので、**手元の時間帯の日付に直してから**見出しにします（§3）——UTC のまま
// 数えると、夜にアーカイブしたカードが翌日の見出しに入ります。

import type { Card } from "../ipc/types/Card";

export interface ArchiveGroup {
  /** 見出し。`YYYY/MM/DD`、日付が無ければ「日付なし」。 */
  label: string;
  cards: Card[];
}

/// アーカイブしたカードを、日ごとにまとめる。
///
/// `matched` は絞り込みに一致したカードの ID（`null` は「絞り込んでいない」）。
/// 一致しなかったものはここで落ちます。
export function archivedGroups(
  cards: readonly Card[],
  matched: ReadonlySet<number> | null,
): ArchiveGroup[] {
  const shown = cards.filter((card) => matched === null || matched.has(card.id));
  // 新しい順。同じ時刻なら ID の順で、並びが揺れないようにする。
  const sorted = [...shown].sort((a, b) => (b.archivedAt ?? 0) - (a.archivedAt ?? 0) || a.id - b.id);

  const groups: ArchiveGroup[] = [];
  for (const card of sorted) {
    const label = archivedDayLabel(card.archivedAt);
    const last = groups.at(-1);
    if (last?.label === label) last.cards.push(card);
    else groups.push({ label, cards: [card] });
  }
  return groups;
}

/// アーカイブした日の見出し。
///
/// `archived_cards` に入っているのに `archivedAt` が無い行は、本来作られません。
/// 出す場所が無くて消えるより、見出しを付けて出します（gpui 版と同じ）。
export function archivedDayLabel(archivedAt: number | null): string {
  if (archivedAt === null) return "日付なし";
  const at = new Date(archivedAt);
  const month = String(at.getMonth() + 1).padStart(2, "0");
  const day = String(at.getDate()).padStart(2, "0");
  return `${String(at.getFullYear())}/${month}/${day}`;
}
