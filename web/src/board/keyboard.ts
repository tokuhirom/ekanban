// キーボードでカードを選び、動かす（§6 の条件 6）。
//
// gpui 版の `next_card_id` と `move_selected_card_between_columns` をそのまま
// 移したものです。**手触りが変わらないことがこの移行の条件**なので、ここで
// 「もっと素直な動き」を発明しません。
//
// dnd-kit の `KeyboardSensor`（掴む → 動かす → 離す の 3 手）は使いません。
// いまの割り当ては修飾キーを押しながら矢印を叩くだけの 1 手で、押している間
// 何枚でも動かせます。3 手に増やすのは手触りを下げることです。

import type { Board } from "../ipc/types/Board";
import { locateCard, type MoveCardArgs } from "./dnd";

export type Direction = "up" | "down" | "left" | "right";

/// 矢印で選択を移したときの、次のカード。
///
/// 左右は、カードのある次のカラムまで飛び越します（空のカラムで止まらない）。
/// 行が足りなければそのカラムの最後のカードを選びます。
export function nextSelection(
  board: Board,
  selected: number | null,
  direction: Direction,
): number | null {
  const firstCard = () =>
    board.columns.flatMap((column) => column.cards).at(0)?.id ?? null;

  if (selected === null) return firstCard();
  const at = locateCard(board, selected);
  if (at === null) return firstCard();

  if (direction === "up") {
    if (at.cardIndex === 0) return null;
    return board.columns[at.columnIndex]?.cards[at.cardIndex - 1]?.id ?? null;
  }
  if (direction === "down") {
    return board.columns[at.columnIndex]?.cards[at.cardIndex + 1]?.id ?? null;
  }

  const step = direction === "left" ? -1 : 1;
  for (let index = at.columnIndex + step; index >= 0 && index < board.columns.length; index += step) {
    const cards = board.columns[index]?.cards ?? [];
    if (cards.length > 0) {
      return cards[Math.min(at.cardIndex, cards.length - 1)]?.id ?? null;
    }
  }
  return null;
}

/// 修飾キー＋矢印で選択中のカードを動かすときの、`move_card` の引数。
///
/// index は動かす前の列に対するもの（`dnd.ts` の `moveCardArgs` と同じ約束）。
/// 下へ 1 つで `+2` になるのはそのためで、書き間違いではありません。
export function keyboardMove(
  board: Board,
  cardId: number,
  direction: Direction,
): MoveCardArgs | null {
  const at = locateCard(board, cardId);
  if (at === null) return null;

  const targetColumnIndex =
    direction === "left"
      ? at.columnIndex - 1
      : direction === "right"
        ? at.columnIndex + 1
        : at.columnIndex;
  const target = board.columns[targetColumnIndex];
  if (targetColumnIndex < 0 || target === undefined) return null;

  const toIndex =
    direction === "up"
      ? Math.max(at.cardIndex - 1, 0)
      : direction === "down"
        ? at.cardIndex + 2
        : Math.min(at.cardIndex, target.cards.length);
  return { toColumnId: target.id, toIndex };
}

/// 入力欄にフォーカスがある間はボードの割り当てを無効にする（`docs/DESIGN.md`）。
///
/// IME の変換中に矢印を取ると、変換候補が選べなくなります。`isComposing` を
/// 見るのは、変換中は要素の種類に関わらず渡さないためです。
export function boardShortcutsDisabled(event: KeyboardEvent): boolean {
  if (event.isComposing) return true;
  const target = event.target;
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement;
}

/// macOS で動いているか。
///
/// `navigator.platform` は非推奨なので UA を見ます。WKWebView の UA には
/// `Macintosh` が入り、WebView2 と WebKitGTK には入りません。ここを間違えると
/// `secondary` が別のキーになり、割り当てが丸ごと効かなくなります。
const IS_MAC = navigator.userAgent.includes("Mac");

/// 「カードを動かす」修飾キーの組み合わせか。
///
/// macOS は Cmd、ほかは Ctrl（`secondary`）に Alt を足したもの。ほかの修飾キーが
/// 混ざっていたら別の割り当てなので、取りません。
export function movesSelectedCard(event: KeyboardEvent): boolean {
  const secondary = IS_MAC ? event.metaKey : event.ctrlKey;
  const other = IS_MAC ? event.ctrlKey : event.metaKey;
  return secondary && event.altKey && !event.shiftKey && !other;
}

export function arrowDirection(key: string): Direction | null {
  switch (key) {
    case "ArrowUp":
      return "up";
    case "ArrowDown":
      return "down";
    case "ArrowLeft":
      return "left";
    case "ArrowRight":
      return "right";
    default:
      return null;
  }
}
