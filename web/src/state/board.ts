// スナップショットを保持し、コマンドを呼んで差し替える 1 本の経路。
//
// 盤面は Rust が持ちます（ADR 0018）。ここが持つのは、その投影と、まだ確定して
// いない表示の状態（検索語、サイドバーの開閉）だけです。**盤面の論理をこちらに
// 書かないこと。** 書いた時点で真実が 2 つになります。

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { moveCardArgs, moveColumnArgs, parseHandle, previewMove } from "../board/dnd";
import { useIpc } from "../ipc";
import { asAppError, describeFailure } from "../ipc/error";
import type { AppError } from "../ipc/types/AppError";
import type { Board } from "../ipc/types/Board";
import type { DueStatus } from "../ipc/types/DueStatus";
import type { Platform } from "../ipc/types/Platform";
import type { Snapshot } from "../ipc/types/Snapshot";
import type { ThemePreference } from "../ipc/types/ThemePreference";
import { applyTheme } from "../shell/theme";

/** カードの編集パネルが開いている対象。新しいカードはまだ ID を持たない。 */
export type Editing = { kind: "new"; columnId: number } | { kind: "card"; cardId: number };

/** ダイアログに出す知らせ。失敗と、書けたファイルの報せ（ADR 0016）。 */
export interface Alert {
  title: string;
  detail: string;
  /** 押せる行き先が 1 つだけあるとき。書き出しの「場所を開く」がこれ。 */
  action?: { label: string; act: () => void };
}

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
  /** キーボードでカードを動かす（`docs/DESIGN.md`「ドラッグ＆ドロップ」の受け入れ条件）。 */
  moveCard: (cardId: number, toColumnId: number, toIndex: number) => void;
  /** 盤面を返さないもの（起動の読み込み、絞り込み、表示の状態）が失敗した理由。
   *
   * 盤面を変えるコマンドの失敗はここに来ません——`run` がダイアログに出します。 */
  failure: string | null;
  /** コマンドを呼び、返った盤面で差し替える 1 本の経路。
   *
   * 返るのは `Validation` の失敗だけです——それは呼んだ入力欄の脇に出すもの
   * なので、呼び元しか置き場所を知りません。それ以外はここでダイアログに
   * 積むので、呼び元は返り値を捨ててかまいません（ADR 0016）。 */
  run: (call: () => Promise<Snapshot>) => Promise<AppError | null>;
  /** ダイアログに出す知らせ。読んだら `dismissAlert` で消す。 */
  alert: Alert | null;
  dismissAlert: () => void;
  /** ダイアログに出す（書き出しやコピーが終わったときの報せ）。 */
  notify: (alert: Alert) => void;
  /** 開いているカードの編集パネル。 */
  editing: Editing | null;
  openCard: (cardId: number) => void;
  /** そのカラムに新しいカードを足す下書きを開く。まだ何も保存しない（`docs/DESIGN.md`「状態の持ち主」）。 */
  newCard: (columnId: number) => void;
  closePanel: () => void;
  /** 保存されているクイックキャプチャの割り当て。無ければ `null`。 */
  quickCaptureShortcut: string | null;
  setQuickCaptureShortcut: (shortcut: string | null) => void;
  /** アーカイブ表示。盤面の代わりに、アーカイブしたカードを並べる（ADR 0010）。 */
  showArchived: boolean;
  toggleArchive: () => void;
  restoreCard: (cardId: number) => void;
  /** タグ整理パネルの開閉。扱うのはボード全体のタグなので、カードのパネルとは別。 */
  tagPanelOpen: boolean;
  toggleTagPanel: () => void;
  /** メニューから開くときはこちら。開いているのにもう一度押して畳まない。 */
  openTagPanel: () => void;
  /** 入力欄の中身。確定していないので Rust には渡していない。 */
  search: string;
  sidebarCollapsed: boolean;
  /** 選ばれているテーマ。「システムに合わせる」の判定は CSS が持つ（`shell/theme.ts`）。 */
  theme: ThemePreference;
  setTheme: (theme: ThemePreference) => void;
  /** 盤面の取り消し・やり直し。入力欄の中の取り消しとは別（`shell/keys.ts`）。 */
  undo: () => void;
  redo: () => void;
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
  const [theme, setThemeValue] = useState<ThemePreference>("system");
  // どの検索語に対する答えかを一緒に持つ。前の検索語の結果で減光すると、
  // 打っている間だけ違うカードが暗くなる。
  const [result, setResult] = useState<{ query: string; ids: ReadonlySet<number> } | null>(null);
  // ドラッグ中だけの盤面。**Rust には渡しません**——離した瞬間に 1 回だけ
  // `move_card` / `move_column` を呼び、返ったスナップショットで置き換えます
  // （`docs/DESIGN.md`「ドラッグ＆ドロップ」、ADR 0018）。
  const [drag, setDrag] = useState<{ activeId: string; original: Board; preview: Board } | null>(
    null,
  );
  const [selectedCard, setSelectedCard] = useState<number | null>(null);
  const [platform, setPlatform] = useState<Platform>("linux");
  const [alert, setAlert] = useState<Alert | null>(null);
  const [requested, setRequested] = useState<Editing | null>(null);
  const [tagPanelOpen, setTagPanelOpen] = useState(false);
  // 表示だけの状態なので、覚えません。次に開いたときは盤面から始めます
  // （gpui 版と同じ）。
  const [showArchived, setShowArchived] = useState(false);
  const [quickCaptureShortcut, setQuickCaptureShortcut] = useState<string | null>(null);

  const report = useCallback(
    (what: string, error: unknown) => {
      const detail = describe(error);
      setFailure(`${what}: ${detail}`);
      void ipc.logFrontendError(`${what}: ${detail}`);
    },
    [ipc],
  );

  // コマンドを呼んで盤面を差し替える 1 本の経路。**盤面を返すコマンドは全部
  // ここを通します**——起動の読み込みだけが例外で、それは差し替えではなく
  // 最初の 1 回だからです。ここを迂回すると、失敗の行き先も一緒に散ります。
  const run = useCallback(
    async (call: () => Promise<Snapshot>): Promise<AppError | null> => {
      try {
        setSnapshot(await call());
        return null;
      } catch (error: unknown) {
        const failure = asAppError(error);
        if (failure !== null && failure.kind === "validation") {
          // 入力欄に返すものは、呼び元しか置き場所を知らない。ここでダイアログに
          // 出すと、打ち直す先から離れたところに理由が出る。
          return failure;
        }
        const alert: Alert = describeFailure(error);
        setAlert(alert);
        void ipc.logFrontendError(`${alert.title}: ${alert.detail}`);
        return failure;
      }
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
        setThemeValue(startup.theme);
        applyTheme(startup.theme);
        setQuickCaptureShortcut(startup.quickCaptureShortcut);
      })
      .catch((error: unknown) => {
        if (!cancelled) report("ボードを読み込めませんでした", error);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc, report]);

  // クイックキャプチャが書いたとき、盤面はこちらが呼んでいないところで変わる
  // （`docs/DESIGN.md`「コマンドとイベント」）。**差し替えは `run` と同じ 1 本**で、届いた盤面をそのまま載せる。
  useEffect(() => ipc.onBoardChanged(setSnapshot), [ipc]);

  // 検索語が変わるたびに一致する ID を Rust に聞く。**同じ判定を
  // TypeScript にもう 1 つ持たない**（`docs/DESIGN.md`「絞り込みと検索」）。返るのは ID の配列だけなので、
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

  // ウィンドウのタイトルは盤面から導く。**文言を組むのは Rust**で
  // （`Snapshot.windowTitle`）、ここはそれを窓に渡すだけ。
  const windowTitle = snapshot?.windowTitle ?? null;
  useEffect(() => {
    if (windowTitle === null) return;
    void ipc.setWindowTitle(windowTitle).catch((error: unknown) => {
      report("ウィンドウのタイトルを変えられませんでした", error);
    });
  }, [ipc, report, windowTitle]);

  const setTheme = useCallback(
    (next: ThemePreference) => {
      setThemeValue(next);
      applyTheme(next);
      void ipc.setThemePreference(next).catch((error: unknown) => {
        report("テーマを覚えられませんでした", error);
      });
    },
    [ipc, report],
  );

  const undo = useCallback(() => {
    void run(() => ipc.undo());
  }, [ipc, run]);

  const redo = useCallback(() => {
    void run(() => ipc.redo());
  }, [ipc, run]);

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
      void run(() => ipc.switchBoard(boardId));
    },
    [ipc, run],
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

        let move: () => Promise<Snapshot>;
        if (handle.kind === "card") {
          const args = moveCardArgs(current.original, current.preview, handle.id);
          if (args === null) return null;
          move = () => ipc.moveCard(handle.id, args.toColumnId, args.toIndex);
        } else {
          const args = moveColumnArgs(current.original, current.preview, handle.id);
          if (args === null) return null;
          move = () => ipc.moveColumn(handle.id, args.toIndex);
        }
        // 保存が通ってから外す。先に外すと、確定した並びが出るまでの一瞬だけ
        // 元の位置に戻って見える（条件 7）。失敗しても外す——動かせなかった
        // ことは、カードが元の位置に戻ることで分かる。
        void run(move).finally(() => {
          setDrag(null);
        });
        return current;
      });
    },
    [ipc, run],
  );

  const moveCard = useCallback(
    (cardId: number, toColumnId: number, toIndex: number) => {
      void run(() => ipc.moveCard(cardId, toColumnId, toIndex));
    },
    [ipc, run],
  );

  // 開いていたカードが消えたら（削除・アーカイブ・別のボードへ切り替え）
  // パネルを畳む。**消えたカードの編集画面を残さない**——保存を押しても
  // 行き先が無い。
  //
  // 状態を書き換えるのではなく、盤面から**導きます**。書き換えると、消えた
  // ことに気づくまでの 1 回ぶん、無い行き先を指したパネルが描かれます。
  const editing =
    requested?.kind === "card" &&
    snapshot !== null &&
    !snapshot.board.columns.some((column) =>
      column.cards.some((card) => card.id === requested.cardId),
    )
      ? null
      : requested;

  const openCard = useCallback((cardId: number) => {
    setSelectedCard(cardId);
    setRequested({ kind: "card", cardId });
  }, []);

  const newCard = useCallback((columnId: number) => {
    setRequested({ kind: "new", columnId });
  }, []);

  const closePanel = useCallback(() => {
    setRequested(null);
  }, []);

  const toggleTagPanel = useCallback(() => {
    setTagPanelOpen((open) => !open);
  }, []);

  const openTagPanel = useCallback(() => {
    setTagPanelOpen(true);
  }, []);

  const dismissAlert = useCallback(() => {
    setAlert(null);
  }, []);

  const notify = useCallback((next: Alert) => {
    setAlert(next);
  }, []);

  const toggleArchive = useCallback(() => {
    setShowArchived((shown) => !shown);
  }, []);

  const restoreCard = useCallback(
    (cardId: number) => {
      void run(() => ipc.restoreCard(cardId));
    },
    [ipc, run],
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
    run,
    alert,
    dismissAlert,
    notify,
    quickCaptureShortcut,
    setQuickCaptureShortcut,
    showArchived,
    toggleArchive,
    restoreCard,
    editing,
    openCard,
    newCard,
    closePanel,
    tagPanelOpen,
    toggleTagPanel,
    openTagPanel,
    search,
    sidebarCollapsed,
    theme,
    setTheme,
    undo,
    redo,
    matched,
    dueStatuses,
    setSearch,
    toggleSidebar,
    switchBoard,
  };
}

/// コマンドが返した `AppError` から、人が読む一行を取り出す。
function describe(error: unknown): string {
  return asAppError(error)?.detail ?? String(error);
}
