// ボード一覧の件数から、その 1 枚へ辿るための並び（#136）。
//
// **判定はここでしません。** 期限が過ぎているかどうかを決めるのは Rust の
// `due_status` で、ここが見るのはその結果（`Snapshot.due_statuses`）だけです
// （`docs/DESIGN.md`「絞り込みと検索」）。

import type { Board } from "../ipc/types/Board";
import type { DueStatus } from "../ipc/types/DueStatus";

/** ボード一覧が数えている 2 つの件数（`DueCounts`）に対応する。 */
export type DueKind = "overdue" | "today";

/// その状態のカードのうち、盤面でいちばん先にあるもの。
///
/// 順はカラムの並び、そのカラムの中のカードの並び。**一覧の件数を押した人が
/// 探しているのは「どれか 1 枚」ではなく「最初の 1 枚」**で、そこから矢印キーで
/// 次へ辿れます。
///
/// **完了扱いのカラムは飛ばします**（ADR 0038）。件数がそこを数えていないので、
/// 辿った先がそこだと、押した数と着いた場所が食い違います。
export function firstDueCard(
  board: Board,
  dueStatuses: ReadonlyMap<number, DueStatus>,
  kind: DueKind,
): number | null {
  for (const column of board.columns) {
    if (column.done) continue;
    for (const card of column.cards) {
      if (dueStatuses.get(card.id)?.kind === kind) return card.id;
    }
  }
  return null;
}
