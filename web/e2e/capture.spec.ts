// クイックキャプチャ（`docs/DESIGN.md`「クイックキャプチャ」）。
//
// **ホットキーそのものはここに出ません。** グローバルな割り当ては OS が押しかたを
// 捕まえるもので、ブラウザからは押せません（そもそも Wayland では使えない、
// [ADR 0012]）。ここで確かめるのは、押されたあとの窓と、入れ先の決まり方です。
// **割り当ては設定画面のもの**になったので、そちらは `settings.spec.ts` が
// 見ます（ADR 0047）。
//
// キャプチャの窓は**別のエントリポイント**（`capture.html`）なので、そのまま
// 開けます。閉じるところだけが本物ではありません——ブラウザに閉じる窓が
// ありません。
//
// [ADR 0012]: ../../../docs/adr/0012-focus-after-quick-capture-on-linux.md

import { expect, test } from "@playwright/test";

import {
  editStoredBoard,
  openBoard,
  openCapture,
  seededBoard,
  startHarness,
  stopHarness,
  storedBoard,
} from "./harness";

import { addCard } from "../src/model/board";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

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
