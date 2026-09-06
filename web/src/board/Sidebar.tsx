import type { BoardSummary } from "../ipc/types/BoardSummary";
import type { DueCounts } from "../ipc/types/DueCounts";

/// ボード一覧の 1 行に出す期限の件数。
///
/// 色だけに意味を持たせないので、数の隣に何の件数かを書く。0 のものは出さない。
function DueCountsView({ counts }: { counts: DueCounts }) {
  if (counts.overdue === 0 && counts.today === 0) return null;
  return (
    <div className="due-counts">
      {counts.overdue > 0 && <span data-tone="danger">⚠ 期限切れ {counts.overdue}</span>}
      {counts.today > 0 && <span data-tone="warning">◷ 今日 {counts.today}</span>}
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
  boards: readonly BoardSummary[];
  currentBoardId: number;
  collapsed: boolean;
  onToggle: () => void;
  onSwitch: (boardId: number) => void;
}

export function Sidebar({ boards, currentBoardId, collapsed, onToggle, onSwitch }: Props) {
  return (
    <nav className="sidebar" data-collapsed={collapsed || undefined} aria-label="ボード一覧">
      <div className="sidebar-header">
        {!collapsed && <h2 className="sidebar-title">ボード</h2>}
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
            <button
              type="button"
              className="board-row"
              data-current={board.id === currentBoardId || undefined}
              onClick={() => {
                onSwitch(board.id);
              }}
              title={collapsed ? board.name : undefined}
            >
              {collapsed ? (
                <span className="rail">
                  <span className="rail-initial">{Array.from(board.name)[0] ?? "?"}</span>
                  <RailMark counts={board.due} />
                </span>
              ) : (
                <>
                  <span className="board-name">{board.name}</span>
                  <DueCountsView counts={board.due} />
                </>
              )}
            </button>
          </li>
        ))}
      </ul>
    </nav>
  );
}
