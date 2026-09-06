import {
  DndContext,
  DragOverlay,
  PointerSensor,
  closestCenter,
  closestCorners,
  useSensor,
  useSensors,
  type CollisionDetection,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
} from "@dnd-kit/core";
import { SortableContext, horizontalListSortingStrategy } from "@dnd-kit/sortable";
import { useEffect } from "react";

import { useBoardState } from "../state/board";
import { CardFace } from "./Card";
import { Column } from "./Column";
import { droppableKind, handleId, locateCard, parseHandle } from "./dnd";
import {
  arrowDirection,
  boardShortcutsDisabled,
  keyboardMove,
  movesSelectedCard,
  nextSelection,
} from "./keyboard";
import { Sidebar } from "./Sidebar";

/// 落とし先の候補を、掴んでいるものと同じ種類だけに絞る。
///
/// カラムの中にカードの並べ替えが入れ子になっているので、絞らないとカラムを
/// 掴んだときにカードが `over` に選ばれ、掴んでも何も起きません。
///
/// カードは角どうしの近さで見ます——中心どうしだと、背の高いカードの上に
/// 小さいカードを重ねたときに入れ替わりが起きない。カラムは幅が揃っていて
/// 縦に長いので、中心どうしのほうが素直に決まります。
const collisionDetection: CollisionDetection = (args) => {
  const kind = droppableKind(String(args.active.id));
  const droppableContainers = args.droppableContainers.filter(
    (container) => droppableKind(String(container.id)) === kind,
  );
  if (kind === "column") return closestCenter({ ...args, droppableContainers });
  // カードは、カード同士に加えてカラムそのもの（空きの部分）にも落とせる。
  return closestCorners({
    ...args,
    droppableContainers: args.droppableContainers,
  });
};

export function Board() {
  const state = useBoardState();
  const { board, selectedCard, selectCard, moveCard } = state;

  // キーボードでの選択と移動（§6 の条件 6）。gpui 版と同じ 1 手の割り当て。
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (board === null || boardShortcutsDisabled(event)) return;
      const direction = arrowDirection(event.key);
      if (direction === null) {
        if (event.key === "Escape") selectCard(null);
        return;
      }

      if (movesSelectedCard(event)) {
        if (selectedCard === null) return;
        const args = keyboardMove(board, selectedCard, direction);
        if (args === null) return;
        event.preventDefault();
        moveCard(selectedCard, args.toColumnId, args.toIndex);
        return;
      }

      // 修飾キーが付いているものは、別の割り当てに譲る。
      if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return;
      const next = nextSelection(board, selectedCard, direction);
      if (next === null) return;
      event.preventDefault();
      selectCard(next);
    }

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [board, moveCard, selectCard, selectedCard]);

  // 掴んだと判定するまでに少し動かす。押しただけでドラッグが始まると、
  // カードを選ぶだけのつもりが動いてしまう。
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  if (state.snapshot === null || board === null) {
    return (
      <div className="loading" role="status">
        {state.failure ?? "読み込んでいます…"}
      </div>
    );
  }

  const { boards } = state.snapshot;
  const columnIds = board.columns.map((column) => handleId({ kind: "column", id: column.id }));
  const draggingHandle = state.dragging === null ? null : parseHandle(state.dragging);
  const draggingAt =
    draggingHandle?.kind === "card" ? locateCard(board, draggingHandle.id) : null;
  const draggingCard =
    draggingAt === null ? null : (board.columns[draggingAt.columnIndex]?.cards[draggingAt.cardIndex] ?? null);
  const draggingColumn =
    draggingHandle?.kind === "column"
      ? (board.columns.find((column) => column.id === draggingHandle.id) ?? null)
      : null;

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
        <DndContext
          sensors={sensors}
          collisionDetection={collisionDetection}
          onDragStart={(event: DragStartEvent) => {
            state.beginDrag(String(event.active.id));
          }}
          onDragOver={(event: DragOverEvent) => {
            state.dragOver(event.over === null ? null : String(event.over.id));
          }}
          onDragEnd={(event: DragEndEvent) => {
            state.endDrag(event.over === null);
          }}
          onDragCancel={() => {
            state.endDrag(true);
          }}
        >
          <div className="board-content">
            <SortableContext items={columnIds} strategy={horizontalListSortingStrategy}>
              {board.columns.map((column) => (
                <Column
                  key={column.id}
                  column={column}
                  tags={board.tags}
                  dueStatuses={state.dueStatuses}
                  matched={state.matched}
                  selectedCard={selectedCard}
                  onSelectCard={selectCard}
                />
              ))}
            </SortableContext>
          </div>
          {/* ゴーストは自分の要素。見た目も追従も OS に取られない（ADR 0020）。 */}
          <DragOverlay dropAnimation={null}>
            {draggingCard !== null && (
              <article className="card card-ghost">
                <CardFace
                  card={draggingCard}
                  tags={board.tags}
                  due={state.dueStatuses.get(draggingCard.id)}
                />
              </article>
            )}
            {draggingColumn !== null && (
              <section className="column column-ghost">
                <header className="column-header">
                  <h2 className="column-name">{draggingColumn.name}</h2>
                  <span className="column-count">{draggingColumn.cards.length} 枚</span>
                </header>
              </section>
            )}
          </DragOverlay>
        </DndContext>
      </main>
    </div>
  );
}
