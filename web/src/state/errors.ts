// 盤面のモデルが断った理由を、画面に出す形にする（[ADR 0016]）。
//
// **行き先を分けるのはここです。** 入力欄の脇に出すもの（打ち直せば直る）と、
// ダイアログに出すもの（画面を更新しないと直らない）を、`kind` で分けます。
// 置き場所が断ったときの形（`AppError`）と同じものを作るので、**受ける側は
// どちらから来たかを気にしません**。
//
// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md

import { fieldFailure } from "../ipc/error";
import type { AppError } from "../ipc/types/AppError";
import type { BoardError } from "../model/board";

/// 断りの見出し。**置き場所が返すものと同じ文言**にします——送る前に断ったか、
/// 送って断られたかで、出る言葉が変わらないように。
const TITLES = {
  card: "カードを操作できませんでした",
  column: "カラムを操作できませんでした",
  tag: "タグを操作できませんでした",
  board: "ボードを操作できませんでした",
} as const;

/// モデルの断りを `AppError` にする。
export function describeBoardError(error: BoardError): AppError {
  switch (error.kind) {
    case "emptyCardTitle":
      return fieldFailure(TITLES.card, "cardTitle", "タイトルを入力してください", null);
    case "emptyColumnName":
      return fieldFailure(TITLES.column, "columnName", "カラム名を入力してください", null);
    case "emptyTagName":
      return fieldFailure(TITLES.tag, "tagName", "タグ名を入力してください", null);
    case "emptyBoardName":
      return fieldFailure(TITLES.board, "boardName", "ボード名を入力してください", null);
    case "duplicateTagName":
      return fieldFailure(
        TITLES.tag,
        "tagName",
        "同じ名前のタグがすでにあります。別の名前を入力してください",
        error.name,
      );
    case "emptyChecklistItemText":
      return fieldFailure(TITLES.card, "checklistItem", "チェック項目を入力してください", null);
    // 以下は打ち直して直るものではないので、ダイアログに出します。画面が
    // 古いまま操作したときに出るもので、更新すれば消えます。
    case "cardNotFound":
      return dialog(TITLES.card, `カード #${String(error.cardId)} が見つかりません。画面を更新してください`);
    case "columnNotFound":
      return dialog(
        TITLES.column,
        `カラム #${String(error.columnId)} が見つかりません。画面を更新してください`,
      );
    case "tagNotFound":
      return dialog(TITLES.tag, `タグ #${String(error.tagId)} が見つかりません。画面を更新してください`);
    case "checklistItemNotFound":
      return dialog(
        TITLES.card,
        `カード #${String(error.cardId)} のチェック項目 #${String(error.itemId)} が見つかりません`,
      );
    case "lastColumn":
      return dialog(TITLES.column, "最後のカラムは削除できません");
  }
}

function dialog(title: string, detail: string): AppError {
  return { kind: "boardIo", title, detail, field: null, value: null };
}
