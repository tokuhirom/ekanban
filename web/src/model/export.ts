// 盤面を人が読む形にする（Markdown）。
//
// **書き出しは 2 つに分かれます**（[ADR 0045]）。ここにあるのは Markdown だけで、
// JSON のほうは置き場所が出します——あちらは**置いてある形の写し**で、採番の
// 続きのように画面が受け取らない値まで入るためです。
//
// [ADR 0045]: ../../../docs/adr/0045-two-kinds-of-export.md

import type { Board } from "../ipc/types/Board";
import type { Card } from "../ipc/types/Card";

/** 書き出す形。 */
export type ExportFormat = "json" | "markdown";

/** 形ごとの拡張子。保存ダイアログに出す名前と、書くときの補いに使う。 */
export const EXTENSION: Record<ExportFormat, string> = { json: "json", markdown: "md" };

/// 保存ダイアログに出す既定のファイル名。
///
/// ファイル名にできない文字（区切りと制御文字）を `_` にし、前後の空白と点を
/// 落とします。**残らなければ `board`**——名前の無いファイルは作れません。
export function suggestedExportName(boardName: string, extension: string): string {
  // 区切り（`/` `\`）と制御文字。どちらもファイル名にすると、置き場所ごと
  // 変わるか、名前として読めないものになります。
  const stem = trimDots(boardName.replaceAll(/[\p{Cc}/\\]/gu, "_").trim());
  return `${stem === "" ? "board" : stem}.${extension}`;
}

/// 盤面を Markdown にする。カラムの順に並べ、アーカイブは最後にまとめる。
export function renderBoardMarkdown(board: Board): string {
  let markdown = `# ${markdownInline(board.name)}\n\n`;
  for (const column of board.columns) {
    markdown += `## ${markdownInline(column.name)}\n\n`;
    if (column.cards.length === 0) {
      markdown += "カードはありません。\n\n";
      continue;
    }
    for (const card of column.cards) {
      markdown += renderCard(card, board, null);
    }
  }

  if (board.archivedCards.length > 0) {
    markdown += "## アーカイブ\n\n";
    for (const card of board.archivedCards) {
      // どのカラムから引き上げたかを添える。盤面から消えているので、
      // カラム名が無いと元の場所が読めません。
      const column = board.columns.find((column) => column.id === card.columnId);
      markdown += renderCard(card, board, column?.name ?? null);
    }
  }

  return markdown;
}

function renderCard(card: Card, board: Board, columnName: string | null): string {
  let markdown = `- **${markdownInline(card.title)}**\n`;

  const metadata: string[] = [];
  if (columnName !== null) metadata.push(`カラム: ${markdownInline(columnName)}`);
  if (card.dueDate !== null) metadata.push(`期限: ${card.dueDate}`);
  const tagNames = card.tagIds
    .map((tagId) => board.tags.find((tag) => tag.id === tagId))
    .filter((tag) => tag !== undefined)
    .map((tag) => markdownInline(tag.name));
  if (tagNames.length > 0) metadata.push(`タグ: ${tagNames.join(", ")}`);
  if (card.archivedAt !== null) metadata.push("アーカイブ済み");
  for (const line of metadata) markdown += `  - ${line}\n`;

  if (card.description.trim() !== "") {
    for (const line of lines(card.description)) markdown += `  > ${markdownInline(line)}\n`;
  }
  for (const item of card.checklistItems) {
    markdown += `  - [${item.checked ? "x" : " "}] ${markdownInline(item.text)}\n`;
  }
  return `${markdown}\n`;
}

/// 打たれた文字を、Markdown の記号として読まれないようにする。
///
/// **改行は空白に潰します。** ここが作るのは 1 行の中に収まる断片なので、
/// 改行が混ざると箇条書きの形が崩れます。
function markdownInline(value: string): string {
  return value
    .replaceAll("\\", "\\\\")
    .replaceAll("\r", " ")
    .replaceAll("\n", " ")
    .replaceAll("*", "\\*")
    .replaceAll("_", "\\_")
    .replaceAll("`", "\\`")
    .replaceAll("[", "\\[")
    .replaceAll("]", "\\]");
}

/// 行に割る。**末尾の改行で空の行を作りません。**
///
/// `split("\n")` をそのまま使うと、`"a\n"` が 2 行（うち 1 行は空）になり、
/// 説明の最後に `> ` だけの行が出ます。
function lines(text: string): string[] {
  const split = text.split("\n");
  if (split.at(-1) === "") split.pop();
  return split.map((line) => (line.endsWith("\r") ? line.slice(0, -1) : line));
}

function trimDots(value: string): string {
  let start = 0;
  let end = value.length;
  while (start < end && value[start] === ".") start += 1;
  while (end > start && value[end - 1] === ".") end -= 1;
  return value.slice(start, end);
}
