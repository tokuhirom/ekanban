// クイックキャプチャ（`docs/DESIGN.md`「クイックキャプチャ」）。
//
// **ホットキーそのものはここに出ません。** グローバルな割り当ては OS が押しかたを
// 捕まえるもので、ブラウザからは押せません（そもそも Wayland では使えない、
// [ADR 0012]）。ここで確かめるのは、押されたあとの窓と、入れ先の決まり方、
// 割り当ての読み書きです。
//
// キャプチャの窓は**別のエントリポイント**（`capture.html`）なので、そのまま
// 開けます。閉じるところだけが本物ではありません——ブラウザに閉じる窓が
// ありません。
//
// [ADR 0012]: ../../../docs/adr/0012-focus-after-quick-capture-on-linux.md

import { expect, test, type Page } from "@playwright/test";

import {
  editStoredBoard,
  openBoard,
  openCapture,
  seededBoard,
  startHarness,
  stopHarness,
  storedBoard,
  storedSetting,
} from "./harness";

import type {} from "../src/ipc/browser";
import { addCard } from "../src/model/board";
import { QUICK_CAPTURE_SHORTCUT } from "../src/store/keys";
import type { AppAction } from "../src/ipc/types/AppAction";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

async function chooseMenu(page: Page, action: AppAction): Promise<void> {
  await page.evaluate((name: AppAction) => {
    window.ekanbanMenu?.(name);
  }, action);
}

test("1 行を打って Enter で、入れ先のカラムの末尾に足される", async ({ page }) => {
  const before = seededBoard();
  const target = before.columns[0];

  await openCapture(page);
  // どこに入るのかを常に見せる。決まっていなければ既定（先頭カラム）。
  await expect(page.locator(".capture-destination")).toHaveText(
    `${before.name} / ${target?.name ?? ""}`,
  );

  await page.locator(".capture-input").fill("思いついたこと");
  await page.locator(".capture-input").press("Enter");

  await expect
    .poll(async () => (await storedBoard(page)).columns[0]?.cards.at(-1)?.title)
    .toBe("思いついたこと");
});

test("窓が開いたら入力欄に焦点があり、そのまま打って Enter で足せる", async ({ page }) => {
  await openCapture(page);

  // ホットキーを押した人が、どこもクリックせずに 1 行打てること。入れ先が
  // 届くまで入力欄は無効なので、有効になったところで焦点が移っている。
  await expect(page.locator(".capture-input")).toBeFocused();

  await page.keyboard.type("クリックせずに打ったこと");
  await page.keyboard.press("Enter");

  await expect
    .poll(async () => (await storedBoard(page)).columns[0]?.cards.at(-1)?.title)
    .toBe("クリックせずに打ったこと");
});

test("空のまま Enter を押しても、何も足さない", async ({ page }) => {
  const before = seededBoard().columns[0]?.cards.length ?? 0;
  await openCapture(page);
  await page.locator(".capture-input").press("Enter");
  // 何も言わずに何も起きない（拒否は黙る、`docs/DESIGN.md`）。
  await expect(page.locator(".capture-hint")).toHaveText("Enter で追加、Escape で閉じる");
  expect((await storedBoard(page)).columns[0]?.cards.length ?? 0).toBe(before);
});

test("入れ先を選ぶと、そのカラムに印が出て、キャプチャもそこへ入る", async ({ page }) => {
  await openBoard(page);
  const second = page.locator(".column").nth(1);
  await second.locator(".column-menu-button").click();
  await second.locator(".set-capture-column").click();

  // 印は ⚡ だけで、意味は読み上げ名が運ぶ（#130）。
  await expect(second.getByRole("img", { name: "クイックキャプチャ先" })).toBeVisible();
  await expect(page.locator(".column").first().locator(".column-capture")).toHaveCount(0);

  await openCapture(page);
  await page.locator(".capture-input").fill("2 つめのカラムへ");
  await page.locator(".capture-input").press("Enter");

  await expect
    .poll(async () => (await storedBoard(page)).columns[1]?.cards.at(-1)?.title)
    .toBe("2 つめのカラムへ");
});

test("ほかの窓が盤面を変えたら、開いているボードにも出る", async ({ page, context }) => {
  await openBoard(page);
  // **本当にもう 1 つの窓から書きます。** 同じ生まれのページが置き場所を
  // 書き換えると、ブラウザが `storage` で教えてくれます——配るアプリで Rust が
  // `board:changed` を投げるのと同じ役目です。
  const columnId = (await storedBoard(page)).columns[0]?.id ?? 0;
  const other = await context.newPage();
  await openCapture(other);
  await editStoredBoard(other, (document) =>
    addCard(document, columnId, "別の窓から足したカード", ""),
  );

  await expect(page.locator(".card", { hasText: "別の窓から足したカード" })).toBeVisible();
});

/// ページの外まで届く割り当ては、この組み立てでは作れない（ADR 0035）。
///
/// **消さずに、理由を出します。** 何ができないのかを画面で読めることが
/// 決めごとで、押せるのに何も起きない状態を作らないためです。
///
/// 割り当てそのもの——押しているキーがその場に出る、記録される、解除できる
/// ——は OS への登録が要るので、**手で確かめます**（ADR 0041 が殻の側に残した
/// 4 つのうちの 1 つ）。押されたキーを文字列にするところは
/// `src/shell/shortcut.test.ts` が見ています。
test("ブラウザでは、割り当てを作れない理由がその場に出る", async ({ page }) => {
  await openBoard(page);
  await chooseMenu(page, "setQuickCaptureShortcut");
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();

  await expect(dialog.locator(".dialog-detail").first()).toContainText("ブラウザ版");

  // 押しても割り当てにはならない。閉じもしない——読む相手はこの理由なので。
  await page.keyboard.press("KeyK");
  await expect(dialog).toBeVisible();
  expect(await storedSetting(page, QUICK_CAPTURE_SHORTCUT)).toBeNull();
});
