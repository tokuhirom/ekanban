// 検索の判定（`docs/DESIGN.md`「絞り込みと検索」）。
//
// 全角半角と大文字小文字を同じものとして扱い、`#12` はカード番号として読みます
// （[ADR 0008]）。**打鍵のたびに呼ばれます**が、盤面は手元にあるので往復しません。
//
// [ADR 0008]: ../../../docs/adr/0008-reaching-a-card-by-its-number.md

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";

/// 検索のために文字を均す。
///
/// 全角の空白を半角に、全角の記号・数字・英字（`！`〜`～`）を半角に落とし、
/// 大文字を小文字にします。**1 文字ずつ小文字にします**——文字列全体に
/// `toLowerCase()` を当てると、前後の文字で結果が変わる規則（ギリシャ語の
/// 語末のシグマ）が混ざり、同じ語が別のものとして扱われることがあります。
export function normalizeSearchText(value: string): string {
  let normalized = "";
  for (const character of value) {
    const code = character.codePointAt(0) ?? 0;
    const mapped =
      code === 0x3000
        ? " "
        : code >= 0xff01 && code <= 0xff5e
          ? String.fromCodePoint(code - 0xfee0)
          : character;
    normalized += mapped.toLowerCase();
  }
  return normalized;
}

/// 検索欄に打たれた `#12` を、カード番号として読む。
///
/// `#` のうしろが数字だけのときにしか効きません。`#イベント` のような、`#` で
/// 始まるだけの普通の検索語はここでは拾わず、文字列検索に落ちます。
export function parseCardNumberQuery(query: string): number | null {
  const normalized = normalizeSearchText(query).trim();
  if (!normalized.startsWith("#")) return null;
  const digits = normalized.slice(1);
  if (digits === "" || !/^\d+$/.test(digits)) return null;
  const id = Number(digits);
  return Number.isSafeInteger(id) ? id : null;
}

/// カード 1 枚が検索語に当たるか。空の検索語はすべてに当たる。
export function cardMatchesSearch(card: Card, query: string): boolean {
  const cardId = parseCardNumberQuery(query);
  if (cardId !== null) return card.id === cardId;
  const normalized = normalizeSearchText(query);
  return (
    normalized === "" ||
    normalizeSearchText(card.title).includes(normalized) ||
    normalizeSearchText(card.description).includes(normalized)
  );
}

/// 検索語とタグに当たるカードの ID。
///
/// **アーカイブしたカードも含めて返します。** 隠すか減光するかの使い分けは
/// 呼ぶ側が決めます（[ADR 0010]）。
///
/// [ADR 0010]: ../../../docs/adr/0010-hiding-instead-of-dimming-in-the-archive.md
export function filterCards(board: Board, query: string, tagId: number | null): number[] {
  const cards: Card[] = [...board.columns.flatMap((column) => column.cards), ...board.archivedCards];
  return cards
    .filter((card) => cardMatchesSearch(card, query))
    .filter((card) => tagId === null || card.tagIds.includes(tagId))
    .map((card) => card.id);
}
