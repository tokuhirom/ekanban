// カードの編集を、Playwright ＋ ハーネスで通す。
//
// 見るのは**画面の状態と SQLite に書かれた内容の両方**です。`invoke()` でハーネスを
// 直接叩いて保存された盤面を読み直します。「画面に出ている」だけでは、保存の
// 配線が抜けていても気づけません。
//
// 動かしているのは本物の webview ではありません（ADR 0021）。エンジンの系統
// （Chromium と WebKit）の差はここで出ますが、platform 層の差は出ません。

import { expect, test, type Page } from "@playwright/test";

import { invoke, openBoard, startHarness, stopHarness } from "./harness";

import type { Snapshot } from "../src/ipc/types/Snapshot";

test.beforeEach(startHarness);
test.afterEach(stopHarness);

/// 保存された盤面を読み直す。画面ではなく SQLite の側を見るための口。
async function storedBoard(): Promise<Snapshot["board"]> {
  const response = await invoke("snapshot");
  const snapshot = (await response.json()) as Snapshot;
  return snapshot.board;
}

async function storedTitles(): Promise<string[]> {
  const board = await storedBoard();
  return board.columns.flatMap((column) => column.cards.map((card) => card.title));
}

/// 1 枚目のカードの編集パネルを開く。クリックは選ぶだけなので、開くのは
/// ダブルクリック（`Card.tsx`）。
async function openFirstCard(page: Page) {
  await page.locator(".column").first().locator(".card").first().dblclick();
  await expect(page.locator(".card-panel")).toBeVisible();
}

// ---------------------------------------------------------------- カード

test("カードを足して保存すると、タイトルがデータベースに入る", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  await page.locator(".card-title-input").fill("新しく足したカード");
  await page.locator(".card-description-input").fill("説明も入れる");
  await page.locator(".save-card").click();

  await expect(page.locator(".card-panel")).toBeHidden();
  await expect.poll(storedTitles).toContain("新しく足したカード");
  const board = await storedBoard();
  const added = board.columns
    .flatMap((column) => column.cards)
    .find((card) => card.title === "新しく足したカード");
  expect(added?.description).toBe("説明も入れる");
  // 末尾に足す（`Board::add_card`）。1 枚目のカラムに入っていること。
  expect(board.columns[0]?.cards.at(-1)?.title).toBe("新しく足したカード");
});

test("説明の欄は、打った分だけ縦に伸びる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  const description = page.locator(".card-description-input");
  const height = async () => (await description.boundingBox())?.height ?? 0;
  const empty = await height();

  await description.fill("一行だけ");
  // 4 行ぶんの下限があるので、少し書いたくらいでは変わらない。
  expect(await height()).toBe(empty);

  await description.fill(Array.from({ length: 20 }, (_, i) => `${String(i)} 行目`).join("\n"));
  await expect.poll(height).toBeGreaterThan(empty);

  // 消せば戻る。伸ばしっぱなしだと、下にある操作が押せなくなる。
  await description.fill("一行だけ");
  await expect.poll(height).toBe(empty);
});

test("タイトル欄で Enter を押すと、そのまま保存される", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".add-card").click();

  // 1 行の欄なので Enter で確定する（`docs/DESIGN.md`）。打ち終わりに保存
  // ボタンまで手を伸ばさせない。
  await page.locator(".card-title-input").fill("Enter で保存");
  await page.locator(".card-title-input").press("Enter");

  await expect(page.locator(".card-panel")).toBeHidden();
  await expect.poll(storedTitles).toContain("Enter で保存");
});

test("足しかけたカードを取り下げると、跡が残らない", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("やっぱりやめる");
  await page.getByRole("button", { name: "キャンセル" }).click();

  await expect(page.locator(".card-panel")).toBeHidden();
  // **一度も存在していない。** 下書きは webview のものなので、そもそも
  // SQLite に触っていない（`docs/DESIGN.md`「状態の持ち主」）。
  expect(await storedTitles()).toEqual(before);
  await expect(page.locator(".card-title", { hasText: "やっぱりやめる" })).toHaveCount(0);
});

test("空のタイトルは、保存されずに断られる", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  await page.locator(".column").first().locator(".add-card").click();
  await page.locator(".card-title-input").fill("   ");

  // 押せるのに断る操作は、理由を言わずにコントロールを無効にする
  // （`docs/DESIGN.md`）。理由は欄の脇に出す。
  await expect(page.locator(".save-card")).toBeDisabled();
  await expect(page.locator(".card-panel .field-error")).toContainText("タイトルを入力してください");
  await page.locator(".card-title-input").press("Enter");
  expect(await storedTitles()).toEqual(before);
});

test("カードを開いて名前を変えると、盤面と保存の両方が変わる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  await page.locator(".card-title-input").fill("書き換えたタイトル");
  await page.locator(".save-card").click();

  await expect(page.locator(".card-panel")).toBeHidden();
  await expect(page.locator(".card-title").first()).toHaveText("書き換えたタイトル");
  await expect.poll(storedTitles).toContain("書き換えたタイトル");
});

test("選んだカードは Enter で開き、Escape で閉じる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".column").first().locator(".card").first().click();
  await expect(page.locator(".card[data-selected]")).toHaveCount(1);

  await page.keyboard.press("Enter");
  await expect(page.locator(".card-panel")).toBeVisible();

  await page.locator(".card-title-input").press("Escape");
  await expect(page.locator(".card-panel")).toBeHidden();
});

// ---------------------------------------------------------------- 期限

test("期限の近道を押すと、その日付が入って保存される", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  await page.getByRole("button", { name: "今日", exact: true }).click();
  // 「今日」は Rust が返した `Snapshot.today` から数える（ブラウザの時計では
  // なく）。ここでもハーネスに聞いて突き合わせる。
  const today = ((await (await invoke("snapshot")).json()) as Snapshot).today;
  await expect(page.locator(".card-due-input")).toHaveValue(today);

  await page.locator(".save-card").click();
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.dueDate,
    )
    .toBe(today);
});

test("読めない期限は、欄の脇で断られる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  await page.locator(".card-due-input").fill("きのう");
  await page.locator(".save-card").click();

  // 断るのは Rust。`Validation` は入力欄の脇に出し、ダイアログには上げない
  // （`docs/DESIGN.md`「コマンドとイベント」、ADR 0016）。値は打ち直せるように残す。
  await expect(page.locator(".card-panel .field-error")).toBeVisible();
  await expect(page.locator(".dialog")).toHaveCount(0);
  await expect(page.locator(".card-panel")).toBeVisible();
  await expect(page.locator(".card-due-input")).toHaveValue("きのう");
});

// ---------------------------------------------------------------- チェックリスト

test("チェックリストの項目を足し、並べ替え、チェックできる", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );

  const before = await page.locator(".checklist-row").count();
  await page.locator(".add-checklist-item").click();
  await page.locator(".checklist-text").nth(before).fill("いちばん目");
  await page.locator(".add-checklist-item").click();
  await page.locator(".checklist-text").nth(before + 1).fill("につ目");

  // 2 つ目を上げると入れ替わる。
  await page.locator(".checklist-row").nth(before + 1).getByLabel("上へ").click();
  await expect(page.locator(".checklist-text").nth(before)).toHaveValue("につ目");

  await page.locator(".checklist-row").nth(before).locator(".checklist-toggle").click();
  await page.locator(".save-card").click();

  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)
        ?.checklistItems.map((item) => [item.text, item.checked]),
    )
    .toEqual([
      ["につ目", true],
      ["いちばん目", false],
    ]);
});

test("中身の無いチェックリスト項目があるうちは保存できない", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);

  await page.locator(".add-checklist-item").click();
  await expect(page.locator(".save-card")).toBeDisabled();
  await expect(page.locator(".checklist-row .field-error")).toContainText(
    "項目名を入力してください",
  );

  await page.locator(".checklist-text").last().fill("書いた");
  await expect(page.locator(".save-card")).toBeEnabled();
});

// ---------------------------------------------------------------- タグ

test("タグを作り、カードに付け、名前を変えて消せる", async ({ page }) => {
  await openBoard(page);
  await page.locator(".open-tag-panel").click();
  await expect(page.locator(".tag-panel")).toBeVisible();

  await page.getByLabel("新しいタグの名前").fill("あたらしいタグ");
  await page.locator(".add-tag").click();
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .toContain("あたらしいタグ");

  // カードのパネルから付ける。
  await page.locator(".open-tag-panel").click();
  await openFirstCard(page);
  const cardId = Number(
    await page.locator(".column").first().locator(".card").first().getAttribute("data-card"),
  );
  await page.locator(".card-tag-picker").getByRole("button", { name: "あたらしいタグ" }).click();
  await page.locator(".save-card").click();

  const tagId = (await storedBoard()).tags.find((tag) => tag.name === "あたらしいタグ")?.id;
  await expect
    .poll(async () =>
      (await storedBoard()).columns
        .flatMap((column) => column.cards)
        .find((card) => card.id === cardId)?.tagIds,
    )
    .toContain(tagId);

  // 名前を変える。1 行の欄なので Enter で確定する。
  await page.locator(".open-tag-panel").click();
  const row = page.locator(`.tag-row[data-tag="${tagId}"]`);
  await row.locator(".tag-name-input").fill("名前を変えたタグ");
  await row.locator(".tag-name-input").press("Enter");
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .toContain("名前を変えたタグ");

  // 消す。カードは残り、付いていたタグが外れるだけ。
  await row.locator(".remove-tag").click();
  await expect
    .poll(async () => (await storedBoard()).tags.map((tag) => tag.name))
    .not.toContain("名前を変えたタグ");
  await expect.poll(storedTitles).not.toHaveLength(0);
});

// ---------------------------------------------------------------- カラム

test("打った名前でカラムが足される", async ({ page }) => {
  await openBoard(page);
  await page.locator(".add-column").click();
  await page.locator(".new-column-name").fill("あたらしいカラム");
  await page.locator(".new-column-name").press("Enter");

  await expect
    .poll(async () => (await storedBoard()).columns.map((column) => column.name))
    .toContain("あたらしいカラム");
  await expect(page.locator(".column-name", { hasText: "あたらしいカラム" })).toBeVisible();
});

test("カラムの名前と WIP 上限を直せる", async ({ page }) => {
  await openBoard(page);
  const column = page.locator(".column").first();
  const columnId = Number(await column.getAttribute("data-column"));

  await column.locator(".column-menu-button").click();
  await column.getByRole("button", { name: "編集" }).click();
  await column.locator(".column-name-input").fill("直した名前");
  await column.locator(".column-wip-input").fill("2");
  await column.locator(".save-column").click();

  await expect
    .poll(async () => {
      const stored = (await storedBoard()).columns.find((each) => each.id === columnId);
      return [stored?.name, stored?.wipLimit];
    })
    .toEqual(["直した名前", 2]);
  // 色だけに意味を持たせない。上限を超えていることは語でも書く。
  await expect(column.locator(".column-over")).toContainText("上限超過");
});

test("読めない WIP 上限は、欄の脇で断られる", async ({ page }) => {
  await openBoard(page);
  const column = page.locator(".column").first();

  await column.locator(".column-menu-button").click();
  await column.getByRole("button", { name: "編集" }).click();
  await column.locator(".column-wip-input").fill("たくさん");
  await column.locator(".save-column").click();

  await expect(column.locator(".field-error")).toBeVisible();
  await expect(page.locator(".dialog")).toHaveCount(0);
});

test("カードの入ったカラムを消すには、確認に答える", async ({ page }) => {
  await openBoard(page);
  const column = page.locator(".column").first();
  const columnId = Number(await column.getAttribute("data-column"));

  await column.locator(".column-menu-button").click();
  await column.getByRole("button", { name: "削除" }).click();

  // 1 操作で複数件が消えるので確認する（`docs/DESIGN.md`）。
  await expect(page.locator(".dialog")).toContainText("カラムを削除しますか？");
  await page.locator(".dialog").getByRole("button", { name: "削除" }).click();

  await expect
    .poll(async () => (await storedBoard()).columns.map((each) => each.id))
    .not.toContain(columnId);
});

test("最後の 1 本になったカラムは消せない", async ({ page }) => {
  // カラムを 1 本だけにするのは画面の外で済ませる。**確かめたいのは、その
  // 状態で削除が押せないこと**で、そこへ辿り着くまでの操作ではない。
  const board = await storedBoard();
  for (const column of board.columns.slice(1)) {
    expect((await invoke("remove_column", { columnId: column.id })).ok).toBe(true);
  }

  await openBoard(page);
  await expect(page.locator(".column")).toHaveCount(1);
  const only = page.locator(".column").first();
  await only.locator(".column-menu-button").click();
  // 理由を言わずにコントロールを無効にする（`docs/DESIGN.md`）。
  await expect(only.getByRole("button", { name: "削除" })).toBeDisabled();
});

// ---------------------------------------------------------------- ボード

test("ボードを足し、名前を変え、消せる", async ({ page }) => {
  await openBoard(page);

  // #91 のとおり、ボード名はインラインではなくダイアログで打つ。
  await page.getByLabel("ボードを追加").click();
  await page.locator(".dialog-input").fill("あたらしいボード");
  await page.locator(".dialog-input").press("Enter");
  await expect(page.locator(".dialog")).toHaveCount(0);
  // 作ったボードがそのまま開く。
  await expect(page.locator(".board-header .board-name")).toHaveText("あたらしいボード");
  await expect.poll(async () => (await storedBoard()).name).toBe("あたらしいボード");

  await page.locator(".rename-board").click();
  await page.locator(".dialog-input").fill("名前を変えたボード");
  await page.locator(".dialog-input").press("Enter");
  await expect(page.locator(".board-header .board-name")).toHaveText("名前を変えたボード");
  await expect.poll(async () => (await storedBoard()).name).toBe("名前を変えたボード");

  const row = page.locator(".board-row-line", { hasText: "名前を変えたボード" });
  await row.getByLabel("名前を変えたボード の操作").click();
  await row.getByRole("button", { name: "削除" }).click();
  await expect(page.locator(".dialog")).toContainText("ボードを削除しますか？");
  await page.locator(".dialog").getByRole("button", { name: "削除" }).click();

  await expect(page.locator(".board-list")).not.toContainText("名前を変えたボード");
});

test("空のボード名は受け付けない", async ({ page }) => {
  await openBoard(page);
  await page.getByLabel("ボードを追加").click();
  await page.locator(".dialog-input").fill("   ");
  await expect(page.locator(".dialog").getByRole("button", { name: "作成" })).toBeDisabled();
});

// ---------------------------------------------------------------- 割り込み

test("入力欄にいる間は、盤面の割り当てを取らない", async ({ page }) => {
  await openBoard(page);
  await openFirstCard(page);
  const before = await storedTitles();

  // パネルのタイトル欄で矢印を叩いても、裏の選択は動かない。
  await page.locator(".card-title-input").click();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowRight");
  await page.waitForTimeout(200);

  expect(await storedTitles()).toEqual(before);
  await expect(page.locator(".card-panel")).toBeVisible();
});

test("失敗はダイアログに出て、盤面はそのまま", async ({ page }) => {
  await openBoard(page);
  const before = await storedTitles();

  // 画面の裏で 1 枚消しておき、同じカードを画面から消しにいく。**入力欄に
  // 返すものではない**失敗なので、ダイアログに出る（ADR 0016）。
  const card = page.locator(".column").first().locator(".card").first();
  const cardId = Number(await card.getAttribute("data-card"));
  expect((await invoke("delete_card", { cardId })).ok).toBe(true);

  await card.click({ button: "right" });
  await page.locator(".card-menu").getByRole("button", { name: "削除" }).click();

  await expect(page.locator(".dialog")).toBeVisible();
  await page.locator(".dialog").getByRole("button", { name: "OK" }).click();
  await expect(page.locator(".dialog")).toHaveCount(0);
  // 消えたのは裏で消した 1 枚だけ。断られた操作は何も変えていない。
  expect((await storedTitles()).length).toBe(before.length - 1);
});
