// スナップショットを保持し、コマンドを呼んで差し替える 1 本の経路。
//
// 盤面は Rust が持ちます（ADR 0018）。ここが持つのは、その投影と、まだ確定して
// いない表示の状態（検索語、サイドバーの開閉）だけです。**盤面の論理をこちらに
// 書かないこと。** 書いた時点で真実が 2 つになります。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { moveCardArgs, moveColumnArgs, parseHandle, previewMove } from "../board/dnd";
import { useIpc } from "../ipc";
import type { Board } from "../ipc/types/Board";
import type { DueStatus } from "../ipc/types/DueStatus";
import type { Platform } from "../ipc/types/Platform";
import type { Snapshot } from "../ipc/types/Snapshot";

export interface BoardState {
  snapshot: Snapshot | null;
  /** ドラッグ中は、動かした先を映した盤面。掴んでいないときは `snapshot` のまま。 */
  board: Board | null;
  /** 掴んでいるものの dnd-kit の ID。ゴーストを描くのに使う。 */
  dragging: string | null;
  selectedCard: number | null;
  /// 動いている OS。Rust から受け取る（UA を見ない）。
  platform: Platform;
  selectCard: (cardId: number | null) => void;
  beginDrag: (activeId: string) => void;
  dragOver: (overId: string | null) => void;
  endDrag: (cancelled: boolean) => void;
  /** キーボードでカードを動かす（§6 の条件 6）。 */
  moveCard: (cardId: number, toColumnId: number, toIndex: number) => void;
  /** 読み込みや操作が失敗した理由。出す場所は呼ぶ側が決める（ADR 0016）。 */
  failure: string | null;
  /** 入力欄の中身。確定していないので Rust には渡していない。 */
  search: string;
  sidebarCollapsed: boolean;
  /** 検索とタグに一致したカード。`null` は「絞り込んでいない」。 */
  matched: ReadonlySet<number> | null;
  dueStatuses: ReadonlyMap<number, DueStatus>;
  setSearch: (value: string) => void;
  toggleSidebar: () => void;
  switchBoard: (boardId: number) => void;
}

export function useBoardState(): BoardState {
  const ipc = useIpc();
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const [search, setSearchValue] = useState("");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  // どの検索語に対する答えかを一緒に持つ。前の検索語の結果で減光すると、
  // 打っている間だけ違うカードが暗くなる。
  const [result, setResult] = useState<{ query: string; ids: ReadonlySet<number> } | null>(null);
  // ドラッグ中だけの盤面。**Rust には渡しません**——離した瞬間に 1 回だけ
  // `move_card` / `move_column` を呼び、返ったスナップショットで置き換えます
  // （§6、ADR 0018）。
  const [drag, setDrag] = useState<{ activeId: string; original: Board; preview: Board } | null>(
    null,
  );
  const [selectedCard, setSelectedCard] = useState<number | null>(null);
  const [platform, setPlatform] = useState<Platform>("linux");

  const report = useCallback(
    (what: string, error: unknown) => {
      const detail = describe(error);
      setFailure(`${what}: ${detail}`);
      void ipc.logFrontendError(`${what}: ${detail}`);
    },
    [ipc],
  );

  useEffect(() => {
    let cancelled = false;
    ipc
      .startupState()
      .then((startup) => {
        if (cancelled) return;
        setSnapshot(startup.snapshot);
        setSearchValue(startup.filter.search);
        setSidebarCollapsed(startup.sidebarCollapsed);
        setPlatform(startup.platform);
      })
      .catch((error: unknown) => {
        if (!cancelled) report("ボードを読み込めませんでした", error);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc, report]);

  // 検索語が変わるたびに一致する ID を Rust に聞く。**同じ判定を
  // TypeScript にもう 1 つ持たない**（§5）。返るのは ID の配列だけなので、
  // 打鍵ごとに呼んでも往復するのはそれだけ。
  //
  // 順番の入れ替わりに備えて、いちばん新しい問い合わせの答えだけを採る。
  const pending = useRef(0);
  useEffect(() => {
    if (snapshot === null || search.trim() === "") return;
    const ticket = ++pending.current;
    ipc
      .filterCards(search, null)
      .then((ids) => {
        if (pending.current === ticket) setResult({ query: search, ids: new Set(ids) });
      })
      .catch((error: unknown) => {
        report("絞り込めませんでした", error);
      });
  }, [ipc, report, search, snapshot]);

  // 答えがまだ返っていない間は絞り込まない。古い答えで減光するより、
  // 一瞬なにも暗くならないほうがよい。
  const matched =
    search.trim() === "" ? null : result?.query === search ? result.ids : null;

  const dueStatuses = useMemo(() => {
    const map = new Map<number, DueStatus>();
    for (const entry of snapshot?.dueStatuses ?? []) {
      map.set(entry.cardId, entry.status);
    }
    return map;
  }, [snapshot]);

  const setSearch = useCallback(
    (value: string) => {
      setSearchValue(value);
      // 覚えるのは確定した値。打鍵ごとに書いてもよいのは、これが
      // `app_state` の 1 行の更新だからで、盤面の保存とは別の経路。
      void ipc.setFilterState({ search: value, tagId: null }).catch((error: unknown) => {
        report("絞り込みを覚えられませんでした", error);
      });
    },
    [ipc, report],
  );

  const toggleSidebar = useCallback(() => {
    setSidebarCollapsed((collapsed) => {
      const next = !collapsed;
      void ipc.setSidebarCollapsed(next).catch((error: unknown) => {
        report("サイドバーの状態を覚えられませんでした", error);
      });
      return next;
    });
  }, [ipc, report]);

  const switchBoard = useCallback(
    (boardId: number) => {
      ipc
        .switchBoard(boardId)
        .then(setSnapshot)
        .catch((error: unknown) => {
        report("ボードを開けませんでした", error);
      });
    },
    [ipc, report],
  );

  const beginDrag = useCallback(
    (activeId: string) => {
      if (snapshot === null) return;
      setDrag({ activeId, original: snapshot.board, preview: snapshot.board });
    },
    [snapshot],
  );

  const dragOver = useCallback((overId: string | null) => {
    if (overId === null) return;
    setDrag((current) => {
      if (current === null) return current;
      const preview = previewMove(current.preview, current.activeId, overId);
      return preview === null ? current : { ...current, preview };
    });
  }, []);

  const endDrag = useCallback(
    (cancelled: boolean) => {
      setDrag((current) => {
        if (current === null) return null;
        if (cancelled) return null;

        const handle = parseHandle(current.activeId);
        if (handle === null) return null;

        let move: Promise<Snapshot>;
        if (handle.kind === "card") {
          const args = moveCardArgs(current.original, current.preview, handle.id);
          if (args === null) return null;
          move = ipc.moveCard(handle.id, args.toColumnId, args.toIndex);
        } else {
          const args = moveColumnArgs(current.original, current.preview, handle.id);
          if (args === null) return null;
          move = ipc.moveColumn(handle.id, args.toIndex);
        }
        move
          .then((next) => {
            setSnapshot(next);
            // 保存が通ってから外す。先に外すと、確定した並びが出るまでの
            // 一瞬だけ元の位置に戻って見える（条件 7）。
            setDrag(null);
          })
          .catch((error: unknown) => {
            report("カードを移動できませんでした", error);
            setDrag(null);
          });
        return current;
      });
    },
    [ipc, report],
  );

  const moveCard = useCallback(
    (cardId: number, toColumnId: number, toIndex: number) => {
      ipc
        .moveCard(cardId, toColumnId, toIndex)
        .then(setSnapshot)
        .catch((error: unknown) => {
          report("カードを移動できませんでした", error);
        });
    },
    [ipc, report],
  );

  return {
    snapshot,
    board: drag?.preview ?? snapshot?.board ?? null,
    dragging: drag?.activeId ?? null,
    selectedCard,
    platform,
    selectCard: setSelectedCard,
    beginDrag,
    dragOver,
    endDrag,
    moveCard,
    failure,
    search,
    sidebarCollapsed,
    matched,
    dueStatuses,
    setSearch,
    toggleSidebar,
    switchBoard,
  };
}

/// コマンドが返した `AppError` から、人が読む一行を取り出す。
function describe(error: unknown): string {
  if (error !== null && typeof error === "object" && "detail" in error) {
    const { detail } = error;
    if (typeof detail === "string") return detail;
  }
  return String(error);
}
