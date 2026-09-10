// テストごとに、空の置き場所と、そこに置いた盤面を 1 つ用意する（[ADR 0041]）。
//
// **プロセスは立てません。** 盤面のモデルは画面と同じ TypeScript にあり
// （[ADR 0039]）、置き場所もブラウザの中です。だから Playwright が動かして
// いるのは**本物のコードそのもの**で、間に挟むものがありません。
//
// 1 つの置き場所を使い回すと、前のテストが動かしたカードの位置に次のテストが
// 引きずられます。**盤面の形が前提になっているテストは順番に依存する**ので、
// テストごとに置き直します。
//
// [ADR 0039]: ../../docs/adr/0039-the-board-model-moves-to-typescript.md
// [ADR 0041]: ../../docs/adr/0041-one-layer-of-screen-tests.md

import { expect, type Page } from "@playwright/test";

import type { AppAction } from "../src/ipc/types/AppAction";
import type { Board } from "../src/ipc/types/Board";
import type { BoardDocument } from "../src/ipc/types/BoardDocument";
import { STORAGE_KEY } from "../src/ipc/local";
// `window.ekanbanMenu` の宣言を読み込むためだけの取り込み（値は使わない）。
import type {} from "../src/ipc/browser";
import type { BoardDocument as ModelDocument, Outcome } from "../src/model/board";
import { cloneDocument } from "../src/model/board";
import { MemoryStore } from "../src/store/memory";
import * as keys from "../src/store/keys";
import { seededState } from "./fixture";

/// 走らせている OS。**ページにこれを名乗らせます。**
///
/// ブラウザに訊かせません——Playwright の WebKit は Linux の上でも `Macintosh`
/// を名乗り、`secondary` が Cmd か Ctrl かを取り違えて `Ctrl+Z` が丸ごと効かなく
/// なります。Playwright の `ControlOrMeta` は**走らせている OS**で決まるので、
/// ここもそれに合わせます。配るアプリで Rust がコンパイル時に知っているのと
/// 同じ答えです（`docs/DESIGN.md`「メニューとキー割り当て」）。
const PLATFORM =
  process.platform === "darwin" ? "macos" : process.platform === "win32" ? "windows" : "linux";

/// ページに付ける問い合わせ。置き場所を `localStorage` に差します。
const QUERY = `?store=local&platform=${PLATFORM}`;

/// テストが始まるときの置き場所。**テストごとに作り直します。**
let stored = "";

export function startHarness(): void {
  stored = JSON.stringify(seededState());
}

export function stopHarness(): void {
  stored = "";
}

/// 蒔いたばかりの、開いているボード。**ページを開く前に読むための口**です
/// （置き場所はまだブラウザに入っていません）。
export function seededBoard(): Board {
  const store = MemoryStore.decode(JSON.stringify(seededState()));
  const document = store.loadDocuments()[0];
  if (document === undefined) throw new Error("蒔いた盤面がない");
  return document.board;
}

/// ページがもう開いているか。
///
/// **開く前の置き場所は Node の側にあります**——`about:blank` に
/// `localStorage` はありません。用意をページの前にしたいテストがあるので
/// （時計を握ってから開く、カラムを 1 本に減らしてから開く）、そのあいだは
/// こちらの写しを読み書きします。
function opened(page: Page): boolean {
  return page.url().startsWith("http");
}

/// 置いてあるものを読む。**画面ではなく保存の側**を見るための口。
async function read(page: Page): Promise<MemoryStore> {
  if (!opened(page)) return MemoryStore.decode(stored);
  const written = await page.evaluate((key) => localStorage.getItem(key), STORAGE_KEY);
  expect(written, "置き場所に盤面がある").not.toBeNull();
  return MemoryStore.decode(written ?? "");
}

/// 置いてあるものを書き換える。次にページを開いたときからこれになります。
async function write(page: Page, store: MemoryStore): Promise<void> {
  stored = store.encode();
  if (!opened(page)) return;
  await page.evaluate(
    ({ key, value }) => {
      localStorage.setItem(key, value);
    },
    { key: STORAGE_KEY, value: stored },
  );
}

/// 置き場所に入っているボードを、全部読む。
export async function storedDocuments(page: Page): Promise<BoardDocument[]> {
  return (await read(page)).loadDocuments();
}

/// 置き場所に入っている、開いているボードの盤面。
///
/// **「画面に出ている」ではなく「保存された」を確かめるための口**です
/// （Rust 側の `Harness::stored` と同じ役目）。
export async function storedBoard(page: Page): Promise<Board> {
  return (await storedOpenDocument(page)).document.board;
}

/// 覚えてある設定を 1 つ読む。鍵は `src/store/keys.ts`。
export async function storedSetting(page: Page, key: string): Promise<string | null> {
  return (await read(page)).get(key);
}

/// 覚えてある設定を 1 つ書き換える。**開く前に置けます**——設定を選んだ状態で
/// 起動するところを見たいテストがあるので（日付の切り替わり、#197）。
export async function editStoredSetting(
  page: Page,
  key: string,
  value: string,
): Promise<void> {
  const store = await read(page);
  store.set(key, value);
  await write(page, store);
}

async function storedOpenDocument(
  page: Page,
): Promise<{ store: MemoryStore; document: BoardDocument }> {
  const store = await read(page);
  const open = Number(store.get(keys.LAST_BOARD) ?? Number.NaN);
  const documents = store.loadDocuments();
  const found = documents.find((document) => document.board.id === open) ?? documents[0];
  if (found === undefined) throw new Error("開いているボードが置き場所にありません");
  return { store, document: found };
}

/// ほかの窓がしたことにして、置き場所の盤面を書き換える。
///
/// **画面と同じモデルを通します**（ADR 0039）。盤面の論理は
/// `web/src/model/board.ts` の 1 つだけなので、テストの用意も同じ道を通ります。
export async function editStoredBoard(
  page: Page,
  act: (document: ModelDocument) => Outcome<unknown>,
): Promise<void> {
  const { store, document: stored } = await storedOpenDocument(page);
  const document = cloneDocument({
    ...stored,
    pendingEvents: [],
    undoStack: [],
    redoStack: [],
  });
  expect(act(document).ok, "モデルが受け付ける").toBe(true);
  store.saveDocument(document, document.pendingEvents);
  await write(page, store);
}

/// メニューの項目が押されたことにする。
///
/// **口が開くのを待ってから呼びます。** `window.ekanbanMenu` を付けるのは React の
/// effect（`src/ipc/browser.ts` の `onAppAction`）なので、ページが `load` を出した
/// あとでも、盤面の `.column` が描かれたあとでも、まだ付いていないことがあります。
/// `page.reload()` の直後がそれで、webkit では 10 回に 1 回ほど当たります。
///
/// **`?.` で呼びません。** 口が無いまま押すと、選んだはずのメニューが黙って捨てられ、
/// 何も起きていない画面を待った先の `expect` が落ちます。落ちる場所と原因が離れると、
/// 環境の遅さで出たり出なかったりする失敗にしか見えません。ここで落とします。
export async function chooseMenu(page: Page, action: AppAction): Promise<void> {
  await page.waitForFunction(() => window.ekanbanMenu !== undefined);
  await page.evaluate((name: AppAction) => {
    const choose = window.ekanbanMenu;
    if (choose === undefined) throw new Error("メニューの口がまだ開いていない");
    choose(name);
  }, action);
}

/// ボードの窓を開く。**開く前に盤面を置きます**——ページが読むより先に
/// `localStorage` へ入れないと、空の置き場所から始まってしまいます。
export async function openBoard(page: Page): Promise<void> {
  await seed(page);
  await page.goto(`/${QUERY}`);
  await expect(page.locator(".column").first()).toBeVisible();
}

/// キャプチャの窓を開く。ボードの窓と同じ置き場所を読みます。
export async function openCapture(page: Page): Promise<void> {
  await seed(page);
  await page.goto(`/capture.html${QUERY}`);
  await expect(page.locator(".capture-input")).toBeVisible();
}

/// ページが読み込まれる前に、置き場所へ盤面を入れておく。
async function seed(page: Page): Promise<void> {
  await page.addInitScript(
    ({ key, initial }) => {
      // **すでに入っていれば触りません。** 同じテストの中で窓を開き直すことが
      // あり（キャプチャの窓）、そこで蒔き直すと 1 つ前の窓が書いたものが消えます。
      if (localStorage.getItem(key) === null) localStorage.setItem(key, initial);
    },
    { key: STORAGE_KEY, initial: stored },
  );
}
