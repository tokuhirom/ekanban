// 書き出し・控えの保存・場所を開く（`docs/DESIGN.md`「アプリが伝えること」）。
//
// **選ぶのは OS のネイティブな保存ダイアログ、書くのは Rust、報せるのは
// アプリの中のダイアログ**、と分けてあります（[ADR 0016]）。中身を作るのは
// 形によって別です——Markdown はここ（`model/export.ts`）、JSON は置き場所
// （[ADR 0045]）。最後のところで OS の
// メッセージダイアログを使わないのは、文言と「場所を開く」の導線を自分で決め
// られること、Playwright から見えること（`docs/DESIGN.md`「テスト」）の 2 つが理由です。
//
// 選ばずに閉じたときは**何も言いません**。拒否・キャンセル・変更なしは黙る、
// という規則のとおりです（`docs/DESIGN.md`）。
//
// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md

import { useCallback, useMemo } from "react";

import { useIpc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { Board } from "../ipc/types/Board";
import type { ExportFormat } from "../model/export";
import { EXTENSION, renderBoardMarkdown, suggestedExportName } from "../model/export";
import type { Alert } from "../state/board";

export interface FileActions {
  exportBoard: (format: ExportFormat) => void;
  backupDatabase: () => void;
  revealDatabase: () => void;
  revealBackups: () => void;
}

export function useFileActions(notify: (alert: Alert) => void, board: Board | null): FileActions {
  const ipc = useIpc();

  /// 保存先を選ばせ、書き、書けた場所を報せる。
  const write = useCallback(
    async (fileName: string, title: string, save: (destination: string) => Promise<string>) => {
      try {
        const destination = await ipc.chooseSavePath(fileName);
        // 選ばずに閉じた。何も言わない。
        if (destination === null) return;
        const written = await save(destination);
        notify({
          title,
          detail: written,
          // 開く相手がいない環境では、この導線ごと出しません（ブラウザ、
          // ADR 0035）。押しても何も起きないボタンは、壊れて見えます。
          ...(ipc.canRevealPaths
            ? {
                action: {
                  label: "場所を開く",
                  act: () => {
                    void ipc.revealPath(written);
                  },
                },
              }
            : {}),
        });
      } catch (error: unknown) {
        notify(describeFailure(error));
      }
    },
    [ipc, notify],
  );

  return useMemo(
    () => ({
      exportBoard: (format) => {
        // 盤面が来る前にメニューから押された。何も言わずに何もしない
        // （`docs/DESIGN.md`「アプリが伝えること」）。
        if (board === null) return;
        const extension = EXTENSION[format];
        void write(
          suggestedExportName(board.name, extension),
          "書き出しました",
          (destination) =>
            format === "markdown"
              ? ipc.writeTextFile(destination, extension, renderBoardMarkdown(board))
              : ipc.exportBoardJson(board.id, destination),
        );
      },
      backupDatabase: () => {
        void write("ekanban-backup.sqlite3", "データベースをコピーしました", (destination) =>
          ipc.backupDatabase(destination),
        );
      },
      revealDatabase: () => {
        void ipc.revealDatabase().catch((error: unknown) => {
          notify(describeFailure(error));
        });
      },
      revealBackups: () => {
        void ipc.revealBackups().catch((error: unknown) => {
          notify(describeFailure(error));
        });
      },
    }),
    [board, ipc, notify, write],
  );
}
