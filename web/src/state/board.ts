// スナップショットを保持し、コマンドを呼んで差し替える 1 本の経路。
//
// 盤面は Rust が持ちます（ADR 0018）。ここが持つのは、その投影と、まだ確定して
// いない表示の状態（検索語、サイドバーの開閉）だけです。**盤面の論理をこちらに
// 書かないこと。** 書いた時点で真実が 2 つになります。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useIpc } from "../ipc";
import type { DueStatus } from "../ipc/types/DueStatus";
import type { Snapshot } from "../ipc/types/Snapshot";

export interface BoardState {
  snapshot: Snapshot | null;
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

  return {
    snapshot,
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
