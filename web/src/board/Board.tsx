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
import { useEffect, useRef, useState } from "react";

import { useIpc } from "../ipc";
import type { AppError } from "../ipc/types/AppError";
import type { BoardSummary } from "../ipc/types/BoardSummary";
import type { Column as ColumnData } from "../ipc/types/Column";
import type { Snapshot } from "../ipc/types/Snapshot";
import { Archive } from "../panel/Archive";
import { CardPanel } from "../panel/CardPanel";
import { TagPanel } from "../panel/TagPanel";
import { useAppActions, useAppActionSource } from "../shell/actions";
import { AlertDialog, ConfirmDialog, PromptDialog } from "../shell/Dialog";
import { useFileActions } from "../shell/files";
import { targetOf, undoIntent } from "../shell/keys";
import { useBoardState } from "../state/board";
import { CardFace, CardMenu } from "./Card";
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

/// 確認ダイアログ 1 回ぶん。出すのは「Undo で戻せない」か「1 操作で複数件が
/// 消える」ものだけ（`docs/DESIGN.md`）。
interface Pending {
  title: string;
  description: string;
  okText: string;
  act: () => void;
}

/// 1 行を打たせるダイアログ 1 回ぶん。ボードの名前がこれ（#91）。
interface Prompt {
  title: string;
  label: string;
  placeholder: string;
  okText: string;
  value: string;
  error: string | null;
  submit: (value: string) => Promise<AppError | null>;
}

export function Board() {
  const state = useBoardState();
  const ipc = useIpc();
  const { board, selectedCard, selectCard, openCard, moveCard, platform, run } = state;
  const [confirming, setConfirming] = useState<Pending | null>(null);
  const [prompt, setPrompt] = useState<Prompt | null>(null);
  const [cardMenu, setCardMenu] = useState<{ cardId: number; x: number; y: number } | null>(null);
  const [addingColumn, setAddingColumn] = useState(false);
  const [about, setAbout] = useState(false);
  const searchInput = useRef<HTMLInputElement>(null);
  const files = useFileActions(state.notify);

  // メニューが押されたことを受けはじめる。配る先はこの下と、開いている
  // パネルの中（`shell/actions.ts`）。
  useAppActionSource();
  useAppActions({
    addBoard: () => {
      askCreateBoard();
    },
    // 選んでいるカードのカラムへ。選んでいなければ先頭のカラムへ（gpui 版と
    // 同じ決め方）。
    addCard: () => {
      // アーカイブ表示にカードを足す場所は無い。理由を言う相手がいないので
      // 黙って何もしない（gpui 版と同じ）。
      if (board === null || state.showArchived) return;
      const column =
        board.columns.find((each) => each.cards.some((card) => card.id === selectedCard)) ??
        board.columns[0];
      if (column !== undefined) state.newCard(column.id);
    },
    addColumn: () => {
      setAddingColumn(true);
    },
    addTag: state.openTagPanel,
    manageTags: state.openTagPanel,
    renameBoard: () => {
      if (board !== null) askRenameBoard(board);
    },
    deleteBoard: () => {
      const summary = state.snapshot?.boards.find((each) => each.id === board?.id);
      if (summary !== undefined) askDeleteBoard(summary);
    },
    // 開いているパネルは自分で畳みます（`CardPanel` と `TagPanel`）。ここが
    // 引き受けるのは、盤面の上に出ているカラムの下書きだけ。
    cancelEdit: () => {
      setAddingColumn(false);
    },
    clearSearch: () => {
      state.setSearch("");
    },
    focusSearch: () => {
      searchInput.current?.focus();
    },
    toggleBoardList: state.toggleSidebar,
    toggleArchiveView: state.toggleArchive,
    exportBoardJson: () => {
      files.exportBoard("json");
    },
    exportBoardMarkdown: () => {
      files.exportBoard("markdown");
    },
    backupDatabase: files.backupDatabase,
    revealDatabase: files.revealDatabase,
    revealBackups: files.revealBackups,
    // メニューからの取り消しも、入力欄にフォーカスがあるときは盤面を巻き戻し
    // ません（gpui 版と同じ）。打っている途中の欄が、下の盤面ごと戻るのを
    // 避けるためです。
    undo: () => {
      if (targetOf(document.activeElement) === "board") state.undo();
    },
    redo: () => {
      if (targetOf(document.activeElement) === "board") state.redo();
    },
    useLightTheme: () => {
      state.setTheme("light");
    },
    useDarkTheme: () => {
      state.setTheme("dark");
    },
    useSystemTheme: () => {
      state.setTheme("system");
    },
    about: () => {
      setAbout(true);
    },
  });

  // キーボードでの選択と移動（§6 の条件 6）。gpui 版と同じ 1 手の割り当て。
  const { undo, redo } = state;
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (board === null) return;

      // 取り消しは入力欄の中でも打たれる。**入力欄では何もしません**——
      // 既定の動きを止めなければ、webview が自分の履歴で取り消します（§7）。
      const intent = undoIntent(event, platform);
      if (intent !== null) {
        if (intent.target === "field") return;
        event.preventDefault();
        if (intent.kind === "undo") undo();
        else redo();
        return;
      }

      if (boardShortcutsDisabled(event)) return;
      const direction = arrowDirection(event.key);
      if (direction === null) {
        // 選んでいるカードを開く。1 回のクリックでは開かないので
        // （`Card.tsx`）、キーボードからの入口はここ。
        if (event.key === "Enter" && selectedCard !== null) {
          event.preventDefault();
          openCard(selectedCard);
        }
        if (event.key === "Escape") selectCard(null);
        return;
      }

      if (movesSelectedCard(event, platform)) {
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
  }, [board, moveCard, openCard, platform, redo, selectCard, selectedCard, undo]);

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
  // ここから下では盤面がある。巻き上げられる関数宣言の中には絞り込みが
  // 届かないので、絞り込んだあとの束縛を 1 つ置く。
  const openBoard = board;
  const columnIds = board.columns.map((column) => handleId({ kind: "column", id: column.id }));
  const draggingHandle = state.dragging === null ? null : parseHandle(state.dragging);
  const draggingAt =
    draggingHandle?.kind === "card" ? locateCard(board, draggingHandle.id) : null;
  const draggingCard =
    draggingAt === null
      ? null
      : (board.columns[draggingAt.columnIndex]?.cards[draggingAt.cardIndex] ?? null);
  const draggingColumn =
    draggingHandle?.kind === "column"
      ? (board.columns.find((column) => column.id === draggingHandle.id) ?? null)
      : null;
  const menuCard =
    cardMenu === null
      ? null
      : (board.columns.flatMap((column) => column.cards).find((card) => card.id === cardMenu.cardId) ??
        null);

  /// カラムをアーカイブする。**空でなければ確認する**——1 操作で複数件が
  /// まとめて動くので（`docs/DESIGN.md`）。
  function askArchiveColumn(column: ColumnData) {
    const act = () => void run(() => ipc.archiveColumn(column.id));
    if (column.cards.length === 0) {
      act();
      return;
    }
    setConfirming({
      title: "カラムをアーカイブしますか？",
      description: `このカラムの ${column.cards.length} 枚のカードをアーカイブします。`,
      okText: "アーカイブ",
      act,
    });
  }

  function askRemoveColumn(column: ColumnData) {
    const act = () => void run(() => ipc.removeColumn(column.id));
    if (column.cards.length === 0) {
      act();
      return;
    }
    setConfirming({
      title: "カラムを削除しますか？",
      description: `このカラムには ${column.cards.length} 枚のカードがあります。削除するとカードも削除されます。`,
      okText: "削除",
      act,
    });
  }

  function askDeleteBoard(summary: BoardSummary) {
    setConfirming({
      title: "ボードを削除しますか？",
      description: `「${summary.name}」と、その中のカードを削除します。`,
      okText: "削除",
      act: () => void run(() => ipc.deleteBoard(summary.id)),
    });
  }

  function askCreateBoard() {
    setPrompt({
      title: "ボードを追加",
      label: "ボードの名前",
      placeholder: "ボードの名前",
      okText: "作成",
      value: "",
      error: null,
      submit: (value) => run(() => ipc.createBoard(value)),
    });
  }

  /// ボードの名前を変える。
  ///
  /// `rename_board` が名前を変えるのは**開いているボード**です（§3）。一覧の
  /// ほかの行から呼ばれたときは、先にそのボードを開きます。
  function askRenameBoard(target: { id: number; name: string }) {
    setPrompt({
      title: "ボードの名前を変更",
      label: "ボードの名前",
      placeholder: "ボードの名前",
      okText: "変更",
      value: target.name,
      error: null,
      submit: async (value) => {
        if (target.id !== openBoard.id) {
          const failure = await run(() => ipc.switchBoard(target.id));
          if (failure !== null) return failure;
        }
        return run(() => ipc.renameBoard(value));
      },
    });
  }

  return (
    <div className="app">
      <Sidebar
        boards={boards}
        currentBoardId={board.id}
        collapsed={state.sidebarCollapsed}
        onToggle={state.toggleSidebar}
        onSwitch={state.switchBoard}
        onCreate={askCreateBoard}
        onRename={askRenameBoard}
        onDelete={askDeleteBoard}
      />
      <main className="board">
        <header className="board-header">
          <h1 className="board-name">{board.name}</h1>
          <button
            type="button"
            className="ghost rename-board"
            aria-label="ボードの名前を変更"
            onClick={() => {
              askRenameBoard(board);
            }}
          >
            ✎
          </button>
          <input
            type="search"
            className="search"
            ref={searchInput}
            value={state.search}
            placeholder="カードを検索 (#12 で番号)"
            aria-label="カードを検索"
            onChange={(event) => {
              state.setSearch(event.target.value);
            }}
          />
          {/* タグの追加・編集・削除はここだけから（`docs/DESIGN.md`）。メニューの
              「タグを整理…」も同じパネルを開きます。 */}
          <button
            type="button"
            className="secondary open-tag-panel"
            aria-pressed={state.tagPanelOpen}
            onClick={state.toggleTagPanel}
          >
            タグ整理
          </button>
          {/* アーカイブの出入り口。件数を出すのは、溜まっていることに気づける
              ようにするため（gpui 版と同じ）。 */}
          <button
            type="button"
            className="secondary archive-view"
            aria-pressed={state.showArchived}
            onClick={state.toggleArchive}
          >
            {state.showArchived
              ? "ボードへ戻る"
              : `アーカイブ (${board.archivedCards.length})`}
          </button>
        </header>
        {state.failure !== null && (
          <p className="failure" role="alert">
            {state.failure}
          </p>
        )}
        {state.showArchived ? (
          <Archive
            board={board}
            dueStatuses={state.dueStatuses}
            matched={state.matched}
            onRestore={state.restoreCard}
          />
        ) : (
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
                  lastColumn={board.columns.length <= 1}
                  run={run}
                  onSelectCard={selectCard}
                  onOpenCard={openCard}
                  onCardContextMenu={(cardId, at) => {
                    setCardMenu({ cardId, ...at });
                  }}
                  onNewCard={state.newCard}
                  onArchiveColumn={askArchiveColumn}
                  onRemoveColumn={askRemoveColumn}
                />
              ))}
            </SortableContext>
            <AddColumn
              // 畳んだときに打ちかけを残さない。開き直したら空から始める。
              key={addingColumn ? "adding" : "idle"}
              open={addingColumn}
              onOpen={() => {
                setAddingColumn(true);
              }}
              onClose={() => {
                setAddingColumn(false);
              }}
              run={run}
            />
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
        )}
      </main>

      {state.editing !== null && !state.showArchived && (
        <CardPanel
          // 対象が変わったら作り直す。下書きを `useEffect` で起こし直すと、
          // 描いてから 1 回ぶん古い値が出る（React の `key` の使いどころ）。
          key={
            state.editing.kind === "card"
              ? `card:${state.editing.cardId}`
              : `new:${state.editing.columnId}`
          }
          board={board}
          editing={state.editing}
          today={state.snapshot.today}
          platform={platform}
          run={run}
          onClose={state.closePanel}
          onArchiveCard={(cardId) => void run(() => ipc.archiveCard(cardId))}
          onDeleteCard={(cardId) => void run(() => ipc.deleteCard(cardId))}
        />
      )}
      {state.tagPanelOpen && (
        <TagPanel tags={board.tags} run={run} onClose={state.toggleTagPanel} />
      )}

      {/* カードの右クリックメニューは盤面の外で描く。カードは dnd-kit の
          `transform` を持つことがあり、それが `position: fixed` の基準に
          なってしまう（`Card.tsx`）。 */}
      {cardMenu !== null && menuCard !== null && (
        <>
          <div
            className="menu-scrim"
            onPointerDown={() => {
              setCardMenu(null);
            }}
            onContextMenu={(event) => {
              event.preventDefault();
              setCardMenu(null);
            }}
          />
          <CardMenu
            card={menuCard}
            tags={board.tags}
            at={{ x: cardMenu.x, y: cardMenu.y }}
            onClose={() => {
              setCardMenu(null);
            }}
            onCopy={() => void run(() => ipc.copyCard(menuCard.id))}
            onArchive={() => void run(() => ipc.archiveCard(menuCard.id))}
            onDelete={() => void run(() => ipc.deleteCard(menuCard.id))}
            onToggleTag={(tagId) => {
              const next = menuCard.tagIds.includes(tagId)
                ? menuCard.tagIds.filter((id) => id !== tagId)
                : [...menuCard.tagIds, tagId];
              void run(() => ipc.setCardTags(menuCard.id, next));
            }}
          />
        </>
      )}

      {prompt !== null && (
        <PromptDialog
          title={prompt.title}
          label={prompt.label}
          placeholder={prompt.placeholder}
          okText={prompt.okText}
          value={prompt.value}
          error={prompt.error}
          onChange={(value) => {
            setPrompt({ ...prompt, value });
          }}
          onCancel={() => {
            setPrompt(null);
          }}
          onOk={() => {
            void prompt.submit(prompt.value).then((failure) => {
              // 断られた値は打ち直せるように残す。通ったときだけ閉じる。
              setPrompt(failure === null ? null : { ...prompt, error: failure.detail });
            });
          }}
        />
      )}
      {confirming !== null && (
        <ConfirmDialog
          title={confirming.title}
          description={confirming.description}
          okText={confirming.okText}
          onCancel={() => {
            setConfirming(null);
          }}
          onOk={() => {
            confirming.act();
            setConfirming(null);
          }}
        />
      )}
      {state.alert !== null && (
        <AlertDialog
          title={state.alert.title}
          detail={state.alert.detail}
          action={state.alert.action}
          onDismiss={state.dismissAlert}
        />
      )}
      {about && (
        <AlertDialog
          title="ekanbanについて"
          detail="ローカル SQLite で動作する Kanban アプリです。"
          onDismiss={() => {
            setAbout(false);
          }}
        />
      )}
    </div>
  );
}

/// カラムを足す枠。カラムの右端に、点線の場所として置く。
///
/// 開いているかどうかは `Board` が持ちます。メニューの「カラムを追加」から
/// 開けるようにするためで、下書きの中身はここに残したままです。
function AddColumn({
  open,
  onOpen,
  onClose,
  run,
}: {
  open: boolean;
  onOpen: () => void;
  onClose: () => void;
  run: (call: () => Promise<Snapshot>) => Promise<AppError | null>;
}) {
  const ipc = useIpc();
  const [name, setName] = useState("");
  const [failed, setFailed] = useState<AppError | null>(null);

  async function save() {
    if (name.trim() === "") return;
    const failure = await run(() => ipc.addColumn(name));
    setFailed(failure);
    if (failure === null) onClose();
  }

  if (!open) {
    return (
      <div className="add-column-placeholder">
        <button type="button" className="secondary add-column" onClick={onOpen}>
          ＋ カラムを追加
        </button>
      </div>
    );
  }

  return (
    <div
      className="add-column-placeholder"
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        // 1 行の欄なので Enter で確定し、Escape で取り消す（`docs/DESIGN.md`）。
        if (event.key === "Enter") {
          event.preventDefault();
          void save();
        } else if (event.key === "Escape") {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      <input
        className="field-input new-column-name"
        value={name}
        placeholder="カラムの名前"
        aria-label="新しいカラムの名前"
        autoFocus
        onChange={(event) => {
          setName(event.target.value);
        }}
      />
      {failed?.field === "columnName" && (
        <p className="field-error" role="alert">
          {failed.detail}
        </p>
      )}
      <div className="button-row">
        <button
          type="button"
          className="primary save-new-column"
          disabled={name.trim() === ""}
          onClick={() => void save()}
        >
          保存
        </button>
        <button type="button" className="secondary" onClick={onClose}>
          取消
        </button>
      </div>
    </div>
  );
}
