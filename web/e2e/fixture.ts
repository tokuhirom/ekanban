// 画面のテストが相手にする盤面（[ADR 0041]）。
//
// **`crates/harness/examples/manual_screenshot_seed.rs` と同じ盤面**です。
// あちらはマニュアルのスクリーンショット用に SQLite を作り、こちらはブラウザの
// 置き場所に置きます。出力が違うので実装も 2 つありますが、**盤面の中身は
// 揃えてあります**——テストが見ている画面と、マニュアルに載る画面が別物だと、
// 説明のほうが嘘になります。
//
// 期限は「撮る日から何日ずらすか」で決めます。期限切れ・今日・近い・それより
// 先が 1 枚ずつ出るようにしてあり、絞り込みと期限の表示を確かめるのに要ります。
//
// [ADR 0041]: ../../docs/adr/0041-one-layer-of-screen-tests.md

import * as keys from "../src/store/keys";
import { boardScopedId } from "../src/store/keys";
import type { StoredBoard, StoredState } from "../src/store/types";

const AT = 1_700_000_000_000;

/// 今日から `offset` 日ずらした日付（`"YYYY-MM-DD"`）。
function day(offset: number): string {
  const at = new Date();
  at.setDate(at.getDate() + offset);
  const pad = (value: number): string => String(value).padStart(2, "0");
  return `${String(at.getFullYear())}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}`;
}

interface Draft {
  columnId: number;
  title: string;
  description: string;
  dueDate: string | null;
  tagIds: number[];
}

function column(id: number, boardId: number, name: string, position: number, done: boolean) {
  return {
    id,
    boardId,
    name,
    position,
    createdAt: AT,
    updatedAt: AT,
    done,
    cards: [],
  };
}

/// カードを順に積む。**採番も `position` もここで合わせます**——置き場所は
/// 受け取った盤面が行として成り立っているかを見るので（ADR 0040）、ずれた
/// 土台は画面ではなく保存のほうを落とします。
function place(board: StoredBoard, drafts: Draft[]): void {
  for (const draft of drafts) {
    const id = board.nextCardId;
    board.nextCardId += 1;
    const target = board.columns.find((each) => each.id === draft.columnId);
    if (target === undefined) throw new Error(`カラム ${String(draft.columnId)} がない`);
    target.cards.push({
      id,
      columnId: draft.columnId,
      title: draft.title,
      description: draft.description,
      position: target.cards.length,
      createdAt: AT,
      updatedAt: AT,
      dueDate: draft.dueDate,
      tagIds: draft.tagIds,
      checklistItems: [],
      archivedAt: null,
      recurrenceId: null,
      occurrenceDate: null,
    });
  }
}

/// 「個人 Kanban」と「家のこと」の 2 つが入った置き場所。
export function seededState(): StoredState {
  const design = 1;
  const research = 2;
  const deadline = 3;

  const personal: StoredBoard = {
    id: 1,
    name: "個人 Kanban",
    createdAt: AT,
    updatedAt: AT,
    nextCardId: 1,
    nextColumnId: 5,
    nextTagId: 4,
    nextChecklistItemId: 4,
    nextRecurrenceId: 4,
    tags: [
      { id: design, boardId: 1, name: "設計", color: "#8b5cf6", createdAt: AT, updatedAt: AT },
      { id: research, boardId: 1, name: "調査", color: "#22c55e", createdAt: AT, updatedAt: AT },
      { id: deadline, boardId: 1, name: "締切あり", color: "#ef4444", createdAt: AT, updatedAt: AT },
    ],
    archivedCards: [],
    columns: [
      column(1, 1, "やること", 0, false),
      column(2, 1, "進行中", 1, false),
      column(3, 1, "完了", 2, true),
      column(4, 1, "寝かせる", 3, false),
    ],
    recurrences: [],
    events: [],
    rev: 0,
  };

  place(personal, [
    {
      columnId: 2,
      title: "SQLite のスキーマを決める",
      description: "差分保存と UPSERT の形を先に決める。",
      dueDate: day(1),
      tagIds: [design, deadline],
    },
    {
      columnId: 1,
      title: "画面の描画をひととおり通す",
      description: "カラムとカードが並ぶところまで。",
      dueDate: day(-2),
      tagIds: [design],
    },
    {
      columnId: 1,
      title: "ドラッグ＆ドロップを試す",
      description: "カラム間の移動と、カラム内の並べ替え。",
      dueDate: day(3),
      tagIds: [research],
    },
    {
      columnId: 1,
      title: "週次の振り返りを書く",
      description: "今週やったことを 10 行でまとめる。",
      dueDate: day(6),
      tagIds: [deadline],
    },
    {
      columnId: 1,
      title: "色のコントラストを確認する",
      description: "ライトとダークの両方で読めるか。",
      dueDate: null,
      tagIds: [],
    },
    {
      columnId: 2,
      title: "キーボード操作を詰める",
      description: "矢印で選び、Ctrl+Alt+矢印で動かす。",
      dueDate: day(0),
      tagIds: [design],
    },
    {
      columnId: 3,
      title: "README を書く",
      description: "何ができるアプリなのかを 1 段落で。",
      dueDate: null,
      tagIds: [],
    },
    {
      columnId: 4,
      title: "URL スキーマの案をためる",
      description: "起動中の 1 つに渡す仕組みが要る。",
      dueDate: null,
      tagIds: [research],
    },
  ]);

  // 編集パネルの確認はこのカードを開く。期限・タグ・チェックリストが全部
  // 埋まっている 1 枚が要るのは、パネルの項目をひととおり出すため。
  const doing = personal.columns[1]?.cards[0];
  if (doing === undefined) throw new Error("進行中のカードがない");
  doing.checklistItems = [
    ["テーブルの列を洗い出す", true],
    ["移行の手順を決める", false],
    ["round-trip のテストを書く", false],
  ].map(([text, checked], at) => ({
    id: at + 1,
    cardId: doing.id,
    text: text as string,
    checked: checked as boolean,
    position: at,
    createdAt: AT,
    updatedAt: AT,
  }));

  // 2 つ目のボード。開いた画面は見ないが、ボード一覧に 2 つ並ぶところが要る。
  // ID はボードごとに区画を取る（`store::board_scoped_id`）。
  const first = boardScopedId(2);
  const home: StoredBoard = {
    id: 2,
    name: "家のこと",
    createdAt: AT,
    updatedAt: AT,
    nextCardId: first,
    nextColumnId: first + 3,
    nextTagId: first,
    nextChecklistItemId: first,
    nextRecurrenceId: first,
    tags: [],
    archivedCards: [],
    columns: [
      column(first, 2, "やること", 0, false),
      column(first + 1, 2, "完了", 1, true),
      column(first + 2, 2, "済み", 2, false),
    ],
    recurrences: [],
    events: [],
    rev: 0,
  };
  place(home, [
    {
      columnId: first,
      title: "洗剤を買う",
      description: "詰め替えの大きいほう。",
      dueDate: null,
      tagIds: [],
    },
    {
      columnId: first,
      title: "自転車の空気を入れる",
      description: "",
      dueDate: null,
      tagIds: [],
    },
  ]);

  return {
    boards: [personal, home],
    state: {
      [keys.LAST_BOARD]: "1",
      [keys.NEXT_BOARD]: "3",
      [keys.CAPTURE_BOARD]: "1",
      [keys.CAPTURE_COLUMN]: "1",
    },
    nextEventId: 1,
    version: 1,
  };
}
