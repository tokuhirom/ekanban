// テストごとに、新しいデータベースと、それを HTTP に出すハーネスを 1 つ立てる。
//
// **通っているのは本物の `ekanban-core`** です（ADR 0021）。画面が読んでいる
// 盤面も、書いた結果も、アプリが起動時に復元するものと同じ経路を通ります。
//
// 1 つのデータベースを使い回すと、前のテストが動かしたカードの位置に次の
// テストが引きずられます。**盤面の形が前提になっているテストは、順番に
// 依存する**ので、そこは分けます（Rust 側の `Harness::open()` と同じ考え）。

import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, type Page } from "@playwright/test";

import type { Board } from "../src/ipc/types/Board";
import type { BoardDocument } from "../src/ipc/types/BoardDocument";
import type { StartupState } from "../src/ipc/types/StartupState";
import type { BoardDocument as ModelDocument, Outcome } from "../src/model/board";
import { cloneDocument } from "../src/model/board";

const ROOT = join(import.meta.dirname, "..", "..");
export const HARNESS_PORT = 1421;

let harness: ChildProcess | undefined;
let workspace: string | undefined;

export async function startHarness(): Promise<void> {
  workspace = mkdtempSync(join(tmpdir(), "ekanban-e2e-"));
  const database = join(workspace, "board.sqlite3");
  // 盤面は SQL ではなくアプリ自身の API で組み立てる。テストが見ている状態が、
  // アプリが本当に復元できる状態であることを、作り方の側で保証するため。
  execFileSync(
    "cargo",
    ["run", "-q", "-p", "ekanban-harness", "--example", "manual_screenshot_seed", "board"],
    { cwd: ROOT, env: { ...process.env, EKANBAN_DATABASE: database }, stdio: "ignore" },
  );

  harness = spawn(
    "cargo",
    ["run", "-q", "-p", "ekanban-harness", "--", database, String(HARNESS_PORT)],
    { cwd: ROOT, stdio: "ignore" },
  );
  for (let i = 0; i < 120; i += 1) {
    try {
      const probe = await invoke("load_documents");
      if (probe.ok) return;
    } catch {
      // まだ立ち上がっていない。
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error("ekanban-harness が立ち上がりませんでした");
}

export async function stopHarness(): Promise<void> {
  harness?.kill();
  harness = undefined;
  // 次のテストが同じポートを取れるまで待つ。
  await new Promise((resolve) => setTimeout(resolve, 250));
  if (workspace !== undefined) rmSync(workspace, { recursive: true, force: true });
  workspace = undefined;
}

/// ハーネスをそのまま叩く。**画面を通さずに SQLite の中身を読む**ための口で、
/// 「画面に出ている」ではなく「保存された」を確かめるのに使います（Rust 側の
/// `Harness::stored_board` と同じ役目）。
export async function invoke(command: string, args: unknown = {}): Promise<Response> {
  return fetch(`http://127.0.0.1:${HARNESS_PORT}/invoke/${command}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(args),
  });
}

/// 置き場所に入っているボードを、全部読む。
export async function storedDocuments(): Promise<BoardDocument[]> {
  const response = await invoke("load_documents");
  expect(response.ok, "load_documents").toBe(true);
  return (await response.json()) as BoardDocument[];
}

/// 起動のときに読むもの（どのボードを開くか、覚えてある設定）。
export async function storedStartup(): Promise<StartupState> {
  const response = await invoke("startup_state");
  expect(response.ok, "startup_state").toBe(true);
  return (await response.json()) as StartupState;
}

/// 置き場所に入っている、開いているボードの盤面。
///
/// **「画面に出ている」ではなく「保存された」を確かめるための口**です
/// （Rust 側の `Harness::stored` と同じ役目）。
export async function storedBoard(): Promise<Board> {
  return (await storedOpenDocument()).board;
}

/// 開いているボードの文書。**無ければテストを落とします**——用意が通って
/// いないのに、盤面の中身を見にいっても意味がありません。
async function storedOpenDocument(): Promise<BoardDocument> {
  const [startup, documents] = await Promise.all([storedStartup(), storedDocuments()]);
  const found = documents.find((document) => document.board.id === startup.openBoardId);
  if (found === undefined) throw new Error("開いているボードが置き場所にありません");
  return found;
}

/// ほかの窓がしたことにして、置き場所の盤面を書き換える。
///
/// **画面と同じモデルを通します**（[ADR 0039]）。盤面の論理は
/// `web/src/model/board.ts` の 1 つだけなので、テストの用意も同じ道を通ります。
///
/// [ADR 0039]: ../../docs/adr/0039-the-board-model-moves-to-typescript.md
export async function editStoredBoard(
  act: (document: ModelDocument) => Outcome<unknown>,
): Promise<void> {
  const stored = await storedOpenDocument();
  const document = cloneDocument({
    ...stored,
    pendingEvents: [],
    undoStack: [],
    redoStack: [],
  });
  expect(act(document).ok, "モデルが受け付ける").toBe(true);
  const saved = await invoke("save_document", {
    document,
    events: document.pendingEvents,
  });
  expect(saved.ok, "save_document").toBe(true);
}

export async function openBoard(page: Page): Promise<void> {
  await page.goto(`/?harness=http://127.0.0.1:${HARNESS_PORT}`);
  await expect(page.locator(".column").first()).toBeVisible();
}
