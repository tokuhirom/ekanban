// 書き出し・控えの保存・場所を開く（§9）。
//
// **選ぶのは OS のネイティブな保存ダイアログ、中身を作るのは Rust、報せるのは
// アプリの中のダイアログ**、と分けてあります（[ADR 0016]）。最後のところで OS の
// メッセージダイアログを使わないのは、文言と「場所を開く」の導線を自分で決め
// られること、Playwright から見えること（§10）の 2 つが理由です。
//
// 選ばずに閉じたときは**何も言いません**。拒否・キャンセル・変更なしは黙る、
// という規則のとおりです（`docs/DESIGN.md`）。
//
// [ADR 0016]: ../../../docs/adr/0016-where-the-app-says-things.md

import { useCallback, useMemo } from "react";

import { useIpc } from "../ipc";
import { describeFailure } from "../ipc/error";
import type { ExportFormat } from "../ipc/types/ExportFormat";
import type { Alert } from "../state/board";

export interface FileActions {
  exportBoard: (format: ExportFormat) => void;
  backupDatabase: () => void;
  revealDatabase: () => void;
  revealBackups: () => void;
}

export function useFileActions(notify: (alert: Alert) => void): FileActions {
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
          action: {
            label: "場所を開く",
            act: () => {
              void ipc.revealPath(written);
            },
          },
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
        void (async () => {
          const suggested = await ipc.suggestedExportName(format).catch(() => "board");
          await write(suggested, "書き出しました", (destination) =>
            ipc.exportBoard(format, destination),
          );
        })();
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
    [ipc, notify, write],
  );
}
