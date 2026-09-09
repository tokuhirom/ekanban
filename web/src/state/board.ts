// 盤面を持ち、当てて、保存する 1 本の経路（[ADR 0039]）。
//
// **盤面はここが持ちます。** 全部のボードを抱えるのは、ボードの切り替えも
// 期限の件数も往復なしで済ませるためです。盤面の論理そのものは
// `web/src/model/board.ts` にあり、ここはそれを当てて `save_document` で
// 書くところです。置き場所が断ったら、当てた写しごと捨てます。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { moveCardArgs, moveColumnArgs, parseHandle, previewMove } from "../board/dnd";
import type { BoardDocument, Outcome } from "../model/board";
import {
  canRedo,
  canUndo,
  cloneDocument,
  moveCard as moveCardIn,
  moveColumn as moveColumnIn,
  redo as redoIn,
  restoreCard as restoreCardIn,
  undo as undoIn,
} from "../model/board";
import type { DueCounts, DueStatus } from "../model/due";
import { dueCounts, dueStatus } from "../model/due";
import { resolveCaptureTarget, type CaptureDestination } from "../model/capture";
import { filterCards } from "../model/search";
import { useIpc } from "../ipc";
import { asAppError, describeFailure } from "../ipc/error";
import type { AppError } from "../ipc/types/AppError";
import type { Board } from "../ipc/types/Board";
import type { CaptureTarget } from "../ipc/types/CaptureTarget";
import type { Platform } from "../ipc/types/Platform";
import type { Tag } from "../ipc/types/Tag";
import type { ThemePreference } from "../ipc/types/ThemePreference";
import { sectionsFor } from "../shell/menu";
import { applyTheme } from "../shell/theme";
import { dayHasTurned, DEFAULT_DAY_BOUNDARY_HOUR, localDay } from "./day";
import { describeBoardError } from "./errors";

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
  snapshot: BoardView | null;
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
  /** 盤面を変えて保存する 1 本の経路（ADR 0039）。
   *
   * 渡すのは**盤面に当てる操作**で、当てるのも保存するのもここです。返るのは
   * `Validation` の失敗だけ——それは呼んだ入力欄の脇に出すものなので、呼び元
   * しか置き場所を知りません。それ以外はここでダイアログに積むので、呼び元は
   * 返り値を捨ててかまいません（ADR 0016）。 */
  run: (
    act: (document: BoardDocument) => Outcome<unknown>,
    boardId?: number,
  ) => Promise<AppError | null>;
  /** 盤面を id から引く。**全部手元にあります**（ADR 0039）。 */
  boardOf: (boardId: number) => Board | null;
  /** 開いているボードのカラムを、クイックキャプチャの入れ先にする。 */
  setCaptureColumn: (columnId: number) => void;
  /** いまのキャプチャ先。**ボードをまたぎます**——開いていないボードを指して
   * いることがあるので、名前まで引いたものを渡す（`model/capture.ts`）。 */
  captureTarget: CaptureDestination | null;
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
  /** 動いているアプリの版と、開いているデータベースのフルパス（#147）。 */
  about: { version: string; databasePath: string };
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
  /** 絞り込んでいるタグの ID。無ければ `null`。 */
  tagId: number | null;
  /** そのタグ本体。盤面から引き直したもので、消えていれば `null`。 */
  activeTag: Tag | null;
  /** カード上のタグチップから絞り込む。同じタグをもう一度で解除。 */
  toggleTag: (tagId: number) => void;
  /** 検索語とタグの両方を解除する。 */
  clearFilter: () => void;
  /** どちらかの条件が効いている。 */
  filtering: boolean;
  sidebarCollapsed: boolean;
  /** 選ばれているテーマ。「システムに合わせる」の判定は CSS が持つ（`shell/theme.ts`）。 */
  theme: ThemePreference;
  setTheme: (theme: ThemePreference) => void;
  /** 日付が変わる時刻（0〜23）。基準日はここから作る（ADR 0048）。 */
  dayBoundaryHour: number;
  setDayBoundaryHour: (hour: number) => void;
  /** 盤面の取り消し・やり直し。入力欄の中の取り消しとは別（`shell/keys.ts`）。 */
  undo: () => void;
  redo: () => void;
  /** 検索とタグに一致したカード。`null` は「絞り込んでいない」。 */
  matched: ReadonlySet<number> | null;
  dueStatuses: ReadonlyMap<number, DueStatus>;
  /** サイドバーに出すボードの一覧。期限の件数は手元で数える（ADR 0011）。 */
  boards: readonly BoardRow[];
  /** 期限を数える基準日（`"YYYY-MM-DD"`）。日付をまたぐと進む。 */
  today: string;
  setSearch: (value: string) => void;
  toggleSidebar: () => void;
  switchBoard: (boardId: number) => void;
  /** ボードを作る。置き場所が採番するので、モデルの操作ではない（ADR 0039）。 */
  createBoard: (name: string) => Promise<AppError | null>;
  deleteBoard: (boardId: number) => Promise<AppError | null>;
}

/// ボード一覧の 1 行。**件数は手元で数えます**（`model/due.ts`）。
///
/// 名前と並びは置き場所から来たものをそのまま使い、件数だけをこちらで出します。
/// 開いているボードは手元の盤面から数えるので、保存していない編集も画面と
/// 合ったままです（[ADR 0011]）。
///
/// [ADR 0011]: ../../../docs/adr/0011-due-counts-in-the-board-list.md
export interface BoardRow {
  id: number;
  name: string;
  createdAt: number;
  updatedAt: number;
  due: DueCounts;
}

/// 画面が読む形。**盤面を持っているのはここ**ですが（[ADR 0039]）、描く側から
/// 見える形はいままでと同じにしてあります。
///
/// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
export interface BoardView {
  board: Board;
  boards: readonly { id: number; name: string; createdAt: number; updatedAt: number }[];
  canUndo: boolean;
  canRedo: boolean;
  /** クイックキャプチャの入れ先が、このボードのどのカラムか。 */
  captureColumn: number | null;
  windowTitle: string;
}

/** 窓に出すアプリの名前。Rust の `ekanban_core::APP_NAME` と同じ綴り。 */
const APP_NAME = "Ekanban";

/// ウィンドウのタイトル。**空白だけの名前ではアプリ名だけ**にします。
function titleOf(boardName: string): string {
  const name = boardName.trim();
  return name === "" ? APP_NAME : `${name} — ${APP_NAME}`;
}

export function useBoardState(): BoardState {
  const ipc = useIpc();
  // **盤面はここが持ちます**（ADR 0039）。全部のボードを抱えるのは、ボードの
  // 切り替えと期限の件数が往復なしで済むからです。
  const [documents, setDocuments] = useState<BoardDocument[]>([]);
  const [openBoardId, setOpenBoardId] = useState<number | null>(null);
  // クイックキャプチャの入れ先。覚えてあるものをそのまま持ちます。**既定に
  // 落とすのはここ**——盤面は手元にあるので、指している先が生きているかどうかを
  // 往復せずに見られます（ADR 0028、ADR 0039）。
  const [storedCaptureTarget, setStoredCaptureTarget] = useState<CaptureTarget | null>(null);
  // 確定を 1 本に並べるための待ち行列と、いまの文書。**描画の写しではなく
  // これを土台にします**——前の確定が飛んでいる間に次が始まると、古い盤面から
  // 作った変更があとから届きます（`CardPanel` の `latest` と同じ形）。
  const documentsRef = useRef<BoardDocument[]>([]);
  const openBoardIdRef = useRef<number | null>(null);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const keep = useCallback((next: BoardDocument[], openId?: number | null) => {
    documentsRef.current = next;
    setDocuments(next);
    if (openId !== undefined) {
      openBoardIdRef.current = openId;
      setOpenBoardId(openId);
    }
  }, []);
  const [failure, setFailure] = useState<string | null>(null);
  const [search, setSearchValue] = useState("");
  // 絞り込んでいるタグ。**検索語と並ぶもう 1 つの条件**で、条件はこの 2 つに
  // 保ちます（`docs/DESIGN.md`「絞り込みと検索」）。
  const [tagId, setTagIdValue] = useState<number | null>(null);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [theme, setThemeValue] = useState<ThemePreference>("system");
  // 日付が変わる時刻（ADR 0048）。起動のときに置き場所から届くまでは既定の 4。
  const [dayBoundaryHour, setDayBoundaryHourValue] = useState(DEFAULT_DAY_BOUNDARY_HOUR);
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
  // 表示だけの状態なので、覚えません。次に開いたときは盤面から始めます。
  const [showArchived, setShowArchived] = useState(false);
  const [quickCaptureShortcut, setQuickCaptureShortcut] = useState<string | null>(null);
  // 版とデータベースの場所は起動のときに 1 回だけ受け取ります（#147）。
  // ダイアログを開くたびに聞き直す理由がありません。
  const [about, setAbout] = useState<{ version: string; databasePath: string }>({
    version: "",
    databasePath: "",
  });

  const report = useCallback(
    (what: string, error: unknown) => {
      const detail = describe(error);
      setFailure(`${what}: ${detail}`);
      void ipc.logFrontendError(`${what}: ${detail}`);
    },
    [ipc],
  );

  /// 置き場所から盤面を読み直す。
  ///
  /// **取り消しの履歴は残します。** 読み直す理由はほかの窓が書いたことで、
  /// こちらが積んだ手が無かったことになるわけではありません。
  const readDocuments = useCallback(
    (openId?: number) => {
      ipc
        .loadDocuments()
        .then((fresh) => {
          const stacks = new Map(
            documentsRef.current.map((document) => [
              document.board.id,
              { undoStack: document.undoStack, redoStack: document.redoStack },
            ]),
          );
          const next = fresh.map((document) => ({
            ...document,
            pendingEvents: [],
            undoStack: stacks.get(document.board.id)?.undoStack ?? [],
            redoStack: stacks.get(document.board.id)?.redoStack ?? [],
          }));
          const open =
            openId ??
            (next.some((document) => document.board.id === openBoardIdRef.current)
              ? openBoardIdRef.current
              : (next[0]?.board.id ?? null));
          keep(next, open);
        })
        .catch((error: unknown) => {
          report("ボードを読み込めませんでした", error);
        });
    },
    [ipc, keep, report],
  );

  /// 盤面を変えて保存する 1 本の経路。**盤面を変える操作は全部ここを通ります。**
  ///
  /// 写しに当ててから保存し、**両方成功してから画面を差し替えます**（ADR 0018
  /// の、この一点は変わりません）。断られたら写しごと捨てるので、画面には何も
  /// 届かず、巻き戻すものもありません。
  ///
  /// 確定は 1 本に並べます。重ねて呼ばれても、土台にするのはいつも 1 つ前の
  /// 結果です。
  const apply = useCallback(
    async (
      act: (document: BoardDocument) => Outcome<unknown>,
      boardId?: number,
    ): Promise<AppError | null> => {
      const target = boardId ?? openBoardIdRef.current;
      const open = documentsRef.current.find((document) => document.board.id === target);
      if (open === undefined) return null;

      const next = cloneDocument(open);
      const outcome = act(next);
      if (!outcome.ok) {
        const failure = describeBoardError(outcome.error);
        // 入力欄に返すものは、呼び元しか置き場所を知らない。ここでダイアログに
        // 出すと、打ち直す先から離れたところに理由が出る（ADR 0016）。
        if (failure.kind === "validation") return failure;
        setAlert(failure);
        void ipc.logFrontendError(`${failure.title}: ${failure.detail}`);
        return failure;
      }
      // 変えていないなら保存しない。何も言わないのは今までどおり。
      if (outcome.value === false) return null;

      const events = next.pendingEvents;
      try {
        const saved = await ipc.saveDocument(next, events);
        next.rev = saved.rev;
        next.pendingEvents = [];
        keep(
          documentsRef.current.map((document) =>
            document.board.id === next.board.id ? next : document,
          ),
        );
        return null;
      } catch (error: unknown) {
        const failure = asAppError(error);
        const alert: Alert = describeFailure(error);
        setAlert(alert);
        void ipc.logFrontendError(`${alert.title}: ${alert.detail}`);
        // 書き負けたときは、置き場所にあるものを読み直します（ADR 0040）。
        // 画面が古いまま押し続けると、断られ続けるだけになります。
        readDocuments();
        return failure;
      }
    },
    [ipc, keep, readDocuments],
  );

  /// 確定を 1 本に並べる。**重ねて呼ばれても、土台はいつも 1 つ前の結果。**
  const run = useCallback(
    (
      act: (document: BoardDocument) => Outcome<unknown>,
      boardId?: number,
    ): Promise<AppError | null> => {
      const queued = queue.current.then(() => apply(act, boardId));
      queue.current = queued;
      return queued;
    },
    [apply],
  );


  useEffect(() => {
    let cancelled = false;
    // **ホットキーの可否はここで聞きません**（ADR 0047）。理由を出す先は設定
    // 画面の割り当ての欄で、メニューの文言ではなくなりました。聞くのは「設定…」
    // を開くときです（`board/Board.tsx`）。
    ipc
      .startupState()
      .then((startup) => {
        if (cancelled) return;
        readDocuments(startup.openBoardId);
        setStoredCaptureTarget(startup.captureTarget);
        setSearchValue(startup.filter.search);
        setTagIdValue(startup.filter.tagId);
        setSidebarCollapsed(startup.sidebarCollapsed);
        setPlatform(startup.platform);
        setThemeValue(startup.theme);
        applyTheme(startup.theme);
        setDayBoundaryHourValue(startup.dayBoundaryHour);
        setQuickCaptureShortcut(startup.quickCaptureShortcut);
        setAbout({ version: startup.version, databasePath: startup.databasePath });

        // メニューバーを掛ける（ADR 0043）。**構成を持っているのはこちら**
        // （`shell/menu.ts`）で、殻はそれを OS のメニューに変換するだけです。
        void ipc.setMenu(sectionsFor(startup.platform));
      })
      .catch((error: unknown) => {
        if (!cancelled) report("ボードを読み込めませんでした", error);
      });
    return () => {
      cancelled = true;
    };
  }, [ipc, readDocuments, report]);


  // クイックキャプチャが書いたとき、盤面はこちらが呼んでいないところで変わる
  // （`docs/DESIGN.md`「コマンドとイベント」）。**差し替えは `run` と同じ 1 本**で、届いた盤面をそのまま載せる。
  // ほかの窓が書いたら読み直す（`docs/DESIGN.md`「コマンドとイベント」）。
  // クイックキャプチャは開いていないボードにも書けるので、全部読み直します。
  useEffect(() => ipc.onBoardChanged(() => { readDocuments(); }), [ipc, readDocuments]);

  // 開きっぱなしで日付をまたいだら、基準日を進める（#135）。
  //
  // **期限の判定はここでします**（`model/due.ts`）。基準日はこの 1 つで、
  // 進めれば「⚠」も件数も一緒に付いてきます。取り直しに行く相手はもう
  // いません。契機は分ごとのタイマーと、窓が見えたとき・前に出たときの 3 つ。
  //
  // **日は 0 時に変わるとは限りません**（ADR 0048）。境界時刻は設定から届く
  // 1 つで、それが変われば基準日もその場で作り直します——`⚠` と件数は同じ
  // 基準日から出ているので、両方いっしょに動きます。
  const [today, setToday] = useState(() => localDay(new Date(), DEFAULT_DAY_BOUNDARY_HOUR));
  useEffect(() => {
    const turn = () => {
      setToday((current) =>
        dayHasTurned(current, new Date(), dayBoundaryHour)
          ? localDay(new Date(), dayBoundaryHour)
          : current,
      );
    };
    // 境界時刻が変わった直後にも合わせる。次の見張りまで古い基準日で待たない。
    turn();
    const timer = setInterval(turn, 60_000);
    document.addEventListener("visibilitychange", turn);
    window.addEventListener("focus", turn);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", turn);
      window.removeEventListener("focus", turn);
    };
  }, [dayBoundaryHour]);

  /// いまのキャプチャ先。決め方はキャプチャの窓と同じ（`model/capture.ts`）。
  const captureTarget = useMemo(
    () => resolveCaptureTarget(documents, storedCaptureTarget),
    [documents, storedCaptureTarget],
  );

  // 画面が読む形。盤面はここが持っているので、組み立てるのもここです。
  const snapshot = useMemo<BoardView | null>(() => {
    const open = documents.find((document) => document.board.id === openBoardId);
    if (open === undefined) return null;
    return {
      board: open.board,
      boards: documents.map((document) => ({
        id: document.board.id,
        name: document.board.name,
        createdAt: document.board.createdAt,
        updatedAt: document.board.updatedAt,
      })),
      canUndo: canUndo(open),
      canRedo: canRedo(open),
      // 入れ先の印は、指しているボードを開いているときだけ出します（ADR 0028）。
      captureColumn:
        captureTarget !== null && captureTarget.boardId === open.board.id
          ? captureTarget.columnId
          : null,
      windowTitle: titleOf(open.board.name),
    };
  }, [captureTarget, documents, openBoardId]);

  // 一致するカードを、盤面が手元にあるうちに数える（`web/src/model/search.ts`）。
  //
  // **打鍵のたびに往復しません。** 判定そのもの——全角半角と大文字小文字の
  // 均し、`#12` のカード番号——は 1 か所（`model/search.ts`）にあり、
  // 盤面もここにあるので、聞きに行く相手がいません。答えを待つ間の
  // 「まだ絞り込めていない」状態も無くなります。
  // いま絞り込んでいるタグ。**盤面から引き直します**——タグを消したり名前を
  // 変えたりしても、ヘッダの表示が古いままにならないように。
  const activeTag = snapshot?.board.tags.find((tag) => tag.id === tagId) ?? null;

  // **消えたタグでは絞り込みません。** 覚えてある `tagId` が指す先が無くなって
  // いると、どのカードも一致せず、盤面が丸ごと暗くなります。起動を妨げない
  // のと同じ扱いで、黙って「絞り込んでいない」に落とします。
  const effectiveTagId = activeTag?.id ?? null;
  const filtering = search.trim() !== "" || effectiveTagId !== null;
  const matched = useMemo(
    () =>
      snapshot === null || !filtering
        ? null
        : new Set(filterCards(snapshot.board, search, effectiveTagId)),
    [effectiveTagId, filtering, search, snapshot],
  );

  // 期限の状態を、盤面が手元にあるうちに出す（`model/due.ts`）。
  //
  // アーカイブ表示も同じ地図を読むので、しまったカードも入れます。
  const dueStatuses = useMemo(() => {
    const map = new Map<number, DueStatus>();
    if (snapshot === null) return map;
    const cards = [
      ...snapshot.board.columns.flatMap((column) => column.cards),
      ...snapshot.board.archivedCards,
    ];
    for (const card of cards) {
      if (card.dueDate !== null) map.set(card.id, dueStatus(card.dueDate, today));
    }
    return map;
  }, [snapshot, today]);

  // ボード一覧。**名前と並びは置き場所から、件数は手元から。**
  //
  // 開いているボードだけは `snapshot` の盤面から数えます——そちらには保存した
  // ばかりの変更が入っているので、一覧と画面が食い違いません。まだ読めていない
  // ボードは 0 件として出します（作ったばかりのボードは実際に 0 件です）。
  const boards = useMemo<BoardRow[]>(() => {
    if (snapshot === null) return [];
    const others = new Map(documents.map((document) => [document.board.id, document.board]));
    return snapshot.boards.map((summary) => {
      const board = summary.id === snapshot.board.id ? snapshot.board : others.get(summary.id);
      return {
        id: summary.id,
        name: summary.name,
        createdAt: summary.createdAt,
        updatedAt: summary.updatedAt,
        due: board === undefined ? { overdue: 0, today: 0 } : dueCounts(board, today),
      };
    });
  }, [documents, snapshot, today]);

  // 絞り込みを覚える。打鍵ごとに書いてもよいのは、これが `app_state` の
  // 1 行の更新だからで、盤面の保存とは別の経路。
  //
  // **2 つの条件をいつも一緒に書きます。** 片方だけ書くと、もう片方が
  // 消えたことになって、次の起動で片肺の絞り込みが戻ります。
  const remember = useCallback(
    (next: { search: string; tagId: number | null }) => {
      void ipc.setFilterState(next).catch((error: unknown) => {
        report("絞り込みを覚えられませんでした", error);
      });
    },
    [ipc, report],
  );

  const setSearch = useCallback(
    (value: string) => {
      setSearchValue(value);
      remember({ search: value, tagId });
    },
    [remember, tagId],
  );

  /// タグでの絞り込みを入れ替える。同じタグをもう一度渡すと解除。
  ///
  /// 押す場所はカードの上のタグチップです（`docs/DESIGN.md`「絞り込みと検索」）。
  /// ヘッダにタグを一覧しないので、**ここが唯一の入口**になります。
  const toggleTag = useCallback(
    (id: number) => {
      const next = tagId === id ? null : id;
      setTagIdValue(next);
      remember({ search, tagId: next });
    },
    [remember, search, tagId],
  );

  /// 絞り込みを両方とも解除する。
  const clearFilter = useCallback(() => {
    setSearchValue("");
    setTagIdValue(null);
    remember({ search: "", tagId: null });
  }, [remember]);

  // ウィンドウのタイトルは盤面から導く。**文言を組むのもここ**（`titleOf`）で、
  // 窓に渡すだけを Rust に頼む。
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

  /// 日付が変わる時刻を選ぶ（ADR 0048）。
  ///
  /// **基準日を作り直すのは見張りの effect** で、この値に依っています。ここで
  /// `today` を触ると、作る場所が 2 つになります。
  const setDayBoundaryHour = useCallback(
    (hour: number) => {
      setDayBoundaryHourValue(hour);
      void ipc.setDayBoundaryHour(hour).catch((error: unknown) => {
        report("日付の切り替わりを覚えられませんでした", error);
      });
    },
    [ipc, report],
  );

  const undo = useCallback(() => {
    void run(undoIn);
  }, [run]);

  const redo = useCallback(() => {
    void run(redoIn);
  }, [run]);

  /// ボードを作る。**採番の名前空間を切るのは置き場所**なので、ここは頼むだけ
  /// （ADR 0039）。返ってきたものを開きます。
  const createBoard = useCallback(
    async (name: string): Promise<AppError | null> => {
      try {
        const created = await ipc.createBoard(name);
        keep(
          [...documentsRef.current, { ...created, pendingEvents: [], undoStack: [], redoStack: [] }],
          created.board.id,
        );
        return null;
      } catch (error: unknown) {
        const failure = asAppError(error);
        if (failure !== null && failure.kind === "validation") return failure;
        setAlert(describeFailure(error));
        return failure;
      }
    },
    [ipc, keep],
  );

  /// ボードを消す。**最後の 1 つは置き場所が断ります。**
  const deleteBoard = useCallback(
    async (boardId: number): Promise<AppError | null> => {
      try {
        await ipc.deleteBoard(boardId);
        const rest = documentsRef.current.filter((document) => document.board.id !== boardId);
        keep(
          rest,
          openBoardIdRef.current === boardId
            ? (rest[0]?.board.id ?? null)
            : openBoardIdRef.current,
        );
        return null;
      } catch (error: unknown) {
        setAlert(describeFailure(error));
        return asAppError(error);
      }
    },
    [ipc, keep],
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
      // 盤面は全部手元にあるので、開くボードを替えるだけ。**置き場所へは
      // 「最後に開いていたボード」を覚えさせに行くだけ**です。
      keep(documentsRef.current, boardId);
      void ipc.setOpenBoard(boardId).catch((error: unknown) => {
        report("開いているボードを覚えられませんでした", error);
      });
    },
    [ipc, keep, report],
  );

  /// 開いているボードのカラムを、クイックキャプチャの入れ先にする。
  const setCaptureColumn = useCallback(
    (columnId: number) => {
      const boardId = openBoardIdRef.current;
      if (boardId === null) return;
      const target = { boardId, columnId };
      setStoredCaptureTarget(target);
      void ipc.setCaptureTarget(target).catch((error: unknown) => {
        report("カードの追加先を覚えられませんでした", error);
      });
    },
    [ipc, report],
  );

  /// 盤面を id から引く。全部手元にあるので、聞きに行く相手がいません。
  const boardOf = useCallback(
    (boardId: number): Board | null =>
      documents.find((document) => document.board.id === boardId)?.board ?? null,
    [documents],
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

        let move: (document: BoardDocument) => Outcome<unknown>;
        if (handle.kind === "card") {
          const args = moveCardArgs(current.original, current.preview, handle.id);
          if (args === null) return null;
          move = (document) => moveCardIn(document, handle.id, args.toColumnId, args.toIndex);
        } else {
          const args = moveColumnArgs(current.original, current.preview, handle.id);
          if (args === null) return null;
          move = (document) => moveColumnIn(document, handle.id, args.toIndex);
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
    [run],
  );

  const moveCard = useCallback(
    (cardId: number, toColumnId: number, toIndex: number) => {
      void run((document) => moveCardIn(document, cardId, toColumnId, toIndex));
    },
    [run],
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
      void run((document) => restoreCardIn(document, cardId));
    },
    [run],
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
    about,
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
    tagId: effectiveTagId,
    activeTag,
    toggleTag,
    clearFilter,
    filtering,
    sidebarCollapsed,
    theme,
    setTheme,
    dayBoundaryHour,
    setDayBoundaryHour,
    undo,
    redo,
    matched,
    dueStatuses,
    boards,
    boardOf,
    setCaptureColumn,
    captureTarget,
    today,
    createBoard,
    deleteBoard,
    setSearch,
    toggleSidebar,
    switchBoard,
  };
}

/// コマンドが返した `AppError` から、人が読む一行を取り出す。
function describe(error: unknown): string {
  return asAppError(error)?.detail ?? String(error);
}
