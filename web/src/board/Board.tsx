import { useBoardState } from "../state/board";
import { Column } from "./Column";
import { Sidebar } from "./Sidebar";

export function Board() {
  const state = useBoardState();

  if (state.snapshot === null) {
    return (
      <div className="loading" role="status">
        {state.failure ?? "読み込んでいます…"}
      </div>
    );
  }

  const { board, boards } = state.snapshot;

  return (
    <div className="app">
      <Sidebar
        boards={boards}
        currentBoardId={board.id}
        collapsed={state.sidebarCollapsed}
        onToggle={state.toggleSidebar}
        onSwitch={state.switchBoard}
      />
      <main className="board">
        <header className="board-header">
          <h1 className="board-name">{board.name}</h1>
          <input
            type="search"
            className="search"
            value={state.search}
            placeholder="カードを検索 (#12 で番号)"
            aria-label="カードを検索"
            onChange={(event) => {
              state.setSearch(event.target.value);
            }}
          />
        </header>
        {state.failure !== null && (
          <p className="failure" role="alert">
            {state.failure}
          </p>
        )}
        <div className="board-content">
          {board.columns.map((column) => (
            <Column
              key={column.id}
              column={column}
              tags={board.tags}
              dueStatuses={state.dueStatuses}
              matched={state.matched}
            />
          ))}
        </div>
      </main>
    </div>
  );
}
