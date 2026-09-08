// クイックキャプチャの入れ先を決める（[ADR 0028]、[ADR 0039]）。
//
// **覚えてあるのは「どのボードのどのカラムか」だけ**です（`capture_target`）。
// それが生きているかを見て、生きていなければ既定に落とすのはこちら側——盤面は
// 画面が全部持っているので、往復せずに決められます。
//
// 決め方が 2 か所に散らないよう、盤面の窓（`state/board.ts`）とキャプチャの窓
// （`capture/Capture.tsx`）の両方がここを通ります。
//
// [ADR 0028]: ../../../docs/adr/0028-a-single-default-quick-capture-target.md
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import type { BoardDocument } from "../ipc/types/BoardDocument";
import type { CaptureTarget } from "../ipc/types/CaptureTarget";

/// 入れ先と、そこに出す名前。
export interface CaptureDestination extends CaptureTarget {
  boardName: string;
  columnName: string;
}

/// いまの入れ先。覚えてあるものが生きていればそれ、無ければ既定。
///
/// 既定は**先頭のボードの先頭カラム**です。開いているボードから決めていたころ
/// は、ボードを切り替えるだけで入れ先が動いていました（#117）。カラムが 1 本も
/// 無ければ入れ先はありません。
export function resolveCaptureTarget(
  documents: readonly BoardDocument[],
  stored: CaptureTarget | null,
): CaptureDestination | null {
  if (stored !== null) {
    const board = documents.find((document) => document.board.id === stored.boardId)?.board;
    const column = board?.columns.find((each) => each.id === stored.columnId);
    if (board !== undefined && column !== undefined) {
      return {
        boardId: board.id,
        columnId: column.id,
        boardName: board.name,
        columnName: column.name,
      };
    }
  }
  const first = documents[0]?.board;
  const column = first?.columns[0];
  return first === undefined || column === undefined
    ? null
    : {
        boardId: first.id,
        columnId: column.id,
        boardName: first.name,
        columnName: column.name,
      };
}
