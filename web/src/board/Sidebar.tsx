import { useState } from "react";

import type { BoardRow } from "../state/board";
import type { DueCounts } from "../model/due";
import type { DueKind } from "./dueOrder";

/// ボード一覧の 1 行に出す期限の件数。
///
/// 色だけに意味を持たせないので、数の隣に何の件数かを書く。0 のものは出さない。
///
/// **押せます**（#136）。押すとそのボードを開き、いちばん先頭の該当カードを
/// 選んで見える位置まで送ります。件数だけ出して辿れないのは行き止まりでした。
/// ボタンはボード名のボタンの外に置きます——ボタンの中にボタンは置けません。
function DueCountsView({
  counts,
  onJump,
}: {
  counts: DueCounts;
  onJump: (kind: DueKind) => void;
}) {
  if (counts.overdue === 0 && counts.today === 0) return null;
  return (
    <div className="due-counts">
      {counts.overdue > 0 && (
        <button
          type="button"
          className="ghost due-jump"
          data-tone="danger"
          onClick={() => {
            onJump("overdue");
          }}
        >
          ⚠ 期限切れ {counts.overdue}
        </button>
      )}
      {counts.today > 0 && (
        <button
          type="button"
          className="ghost due-jump"
          data-tone="warning"
          onClick={() => {
            onJump("today");
          }}
        >
          ◷ 今日 {counts.today}
        </button>
      )}
    </div>
  );
}

/// 畳んだ帯に出す印。幅が狭いので記号と数だけ。過ぎているほうを先に出す。
function RailMark({ counts }: { counts: DueCounts }) {
  if (counts.overdue > 0) return <span data-tone="danger">⚠{counts.overdue}</span>;
  if (counts.today > 0) return <span data-tone="warning">◷{counts.today}</span>;
  return null;
}

interface Props {
  boards: readonly BoardRow[];
  currentBoardId: number;
  collapsed: boolean;
  onToggle: () => void;
  onSwitch: (boardId: number) => void;
  onCreate: () => void;
  onRename: (board: BoardRow) => void;
  onDelete: (board: BoardRow) => void;
  /** 件数が押された。そのボードの、その状態の先頭カードへ送る（#136）。 */
  onJumpDue: (boardId: number, kind: DueKind) => void;
}

export function Sidebar({
  boards,
  currentBoardId,
  collapsed,
  onToggle,
  onSwitch,
  onCreate,
  onRename,
  onDelete,
  onJumpDue,
}: Props) {
  // どの行のメニューが開いているか。1 つだけ開く。
  const [menuFor, setMenuFor] = useState<number | null>(null);

  return (
    <nav className="sidebar" data-collapsed={collapsed || undefined} aria-label="ボード一覧">
      <div className="sidebar-header">
        {!collapsed && <h2 className="sidebar-title">ボード</h2>}
        {/* 畳んだ帯には出しません。追加・名前変更・削除はメニューからも
            届きます（`docs/DESIGN.md`「メニューとキー割り当て」）。 */}
        {!collapsed && (
          <button
            type="button"
            className="ghost add-board"
            aria-label="ボードを追加"
            onClick={onCreate}
          >
            ＋
          </button>
        )}
        <button
          type="button"
          className="sidebar-toggle"
          onClick={onToggle}
          aria-expanded={!collapsed}
          title={collapsed ? "ボード一覧を開く" : "ボード一覧を畳む"}
        >
          {collapsed ? "›" : "‹"}
        </button>
      </div>
      <ul className="board-list">
        {boards.map((board) => (
          <li key={board.id}>
            <div className="board-row-line">
              <button
                type="button"
                className="board-row"
                data-current={board.id === currentBoardId || undefined}
                onClick={() => {
                  onSwitch(board.id);
                }}
                // 名前をダブルクリックしたら名前変更（#119）。開くのは `…` の
                // 「名前を変更」と同じダイアログで、入口だけを増やします。
                // 1 回目のクリックでそのボードに切り替わるので、別の行を
                // ダブルクリックしても切り替えたうえで開きます。
                onDoubleClick={() => {
                  onRename(board);
                }}
                title={collapsed ? board.name : undefined}
              >
                {collapsed ? (
                  <span className="rail">
                    <span className="rail-initial">{Array.from(board.name)[0] ?? "?"}</span>
                    <RailMark counts={board.due} />
                  </span>
                ) : (
                  <span className="board-name">{board.name}</span>
                )}
              </button>
              {!collapsed && (
                <button
                  type="button"
                  className="ghost board-menu-button"
                  aria-label={`${board.name} の操作`}
                  aria-expanded={menuFor === board.id}
                  onClick={() => {
                    setMenuFor((open) => (open === board.id ? null : board.id));
                  }}
                >
                  …
                </button>
              )}
              {menuFor === board.id && (
                <div className="menu board-menu">
                  <button
                    type="button"
                    className="ghost"
                    onClick={() => {
                      setMenuFor(null);
                      onRename(board);
                    }}
                  >
                    名前を変更
                  </button>
                  {/* 最後の 1 つは消せない。理由を言わずにコントロールを無効に
                      する（`docs/DESIGN.md`）。確認に「削除」と答えさせてから
                      断るのは順番が逆。 */}
                  <button
                    type="button"
                    className="danger-item"
                    disabled={boards.length <= 1}
                    onClick={() => {
                      setMenuFor(null);
                      onDelete(board);
                    }}
                  >
                    削除
                  </button>
                </div>
              )}
            </div>
            {/* 件数は行の外。ボタンの中にボタンは置けない（#136）。 */}
            {!collapsed && (
              <DueCountsView
                counts={board.due}
                onJump={(kind) => {
                  onJumpDue(board.id, kind);
                }}
              />
            )}
          </li>
        ))}
      </ul>
    </nav>
  );
}
