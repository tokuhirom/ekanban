// 繰り返しの定義パネル（#198、[ADR 0049]）。
//
// タグ整理パネルと同じ「ボード全体のもの」です（`docs/DESIGN.md`「画面の作り」）
// ——扱うのは開いているボードの繰り返しなので、アプリ全体の設定画面ではなく、
// 右に押し出すパネルに置きます。入り口はメニューの「繰り返しを設定…」1 つ。
//
// **周期の意味はここにありません。** いつ出していつ片付けるかは
// `web/src/model/recurrence.ts` にあり、ここが持っているのは打つ欄と、打った
// ものを `model/board.ts` の操作へ渡すところだけです（[ADR 0039]）。
//
// [ADR 0039]: ../../../docs/adr/0039-the-board-model-moves-to-typescript.md
// [ADR 0049]: ../../../docs/adr/0049-recurring-cards-are-defined-apart-from-the-board.md

import { useState } from "react";

import type { AppError } from "../ipc/types/AppError";
import type { Column } from "../ipc/types/Column";
import type { PreviousPolicy } from "../ipc/types/PreviousPolicy";
import type { Recurrence } from "../ipc/types/Recurrence";
import type { Schedule } from "../ipc/types/Schedule";
import type { Tag } from "../ipc/types/Tag";
import type { BoardDocument, Outcome, RecurrenceDraft } from "../model/board";
import { addRecurrence, addTag, removeRecurrence, updateRecurrence } from "../model/board";
import { describeSchedule, leadDaysOf } from "../model/recurrence";
import { useAppActions } from "../shell/actions";
import { isComposing } from "../shell/ime";
import { AUTO_TAG_COLOR } from "./tags";
import { TagsInput } from "./TagsInput";

type Run = (act: (document: BoardDocument) => Outcome<unknown>) => Promise<AppError | null>;

interface Props {
  recurrences: readonly Recurrence[];
  columns: readonly Column[];
  tags: readonly Tag[];
  run: Run;
  onClose: () => void;
}

/// 周期の種類。**並びは `Schedule` の順**にそろえます。
const KINDS: { kind: Schedule["kind"]; label: string }[] = [
  { kind: "daily", label: "毎日" },
  { kind: "weekday", label: "平日" },
  { kind: "weekly", label: "毎週" },
  { kind: "monthly", label: "毎月" },
  { kind: "monthlyLast", label: "毎月末" },
];

const WEEKDAYS = ["月", "火", "水", "木", "金", "土", "日"];

/// 前回の片付け方。**既定は「アーカイブ」**——取り消せないほうを既定にしません。
const POLICIES: { value: PreviousPolicy; label: string }[] = [
  { value: "archive", label: "アーカイブする" },
  { value: "delete", label: "削除する" },
  { value: "keep", label: "そのまま残す" },
];

export function RecurrencePanel({ recurrences, columns, tags, run, onClose }: Props) {
  const [title, setTitle] = useState("");
  const [failed, setFailed] = useState<AppError | null>(null);

  useAppActions({ cancelEdit: onClose });

  async function add() {
    if (title.trim() === "") return;
    // タグ整理パネルと同じ形で、**送る前に空にします**（#185）。往復を待って
    // から空にすると、待っているあいだに打った文字が巻き込まれて消えます。
    setTitle("");
    const failure = await run((document) =>
      addRecurrence(document, {
        title,
        description: "",
        // 入れ先の既定は一番左。生成のときに消えていたら、そこでも一番左に
        // 落ちます（`model/recurrence.ts` の `targetColumnId`）。
        columnId: columns[0]?.id ?? 0,
        tagIds: [],
        checklist: [],
        schedule: { kind: "daily" },
        leadDays: 0,
        previous: "archive",
        enabled: true,
      }),
    );
    setFailed(failure);
    if (failure !== null) setTitle((current) => (current === "" ? title : current));
  }

  return (
    <aside
      className="panel recurrence-panel"
      aria-label="繰り返しの設定"
      onKeyDown={(event) => {
        // 変換を取り消す Escape でパネルを閉じない（`shell/ime.ts`）。
        if (event.key !== "Escape" || isComposing(event.nativeEvent)) return;
        event.stopPropagation();
        onClose();
      }}
    >
      <header className="panel-header">
        <div className="panel-heading">
          <span className="panel-context">このボード</span>
          <strong className="panel-title">繰り返しの設定</strong>
        </div>
        <div className="panel-actions">
          <button type="button" className="ghost" aria-label="閉じる" onClick={onClose}>
            ✕
          </button>
        </div>
      </header>

      <div className="panel-body">
        {recurrences.length === 0 && (
          <p className="field-note">繰り返しはまだありません。</p>
        )}
        {recurrences.map((recurrence) => (
          <RecurrenceRow
            key={recurrence.id}
            recurrence={recurrence}
            columns={columns}
            tags={tags}
            run={run}
          />
        ))}

        <span className="field-label">繰り返しを追加</span>
        <div className="recurrence-add">
          <input
            className="field-input recurrence-title-input"
            value={title}
            placeholder="カードの題"
            aria-label="新しい繰り返しの題"
            // メニューの「繰り返しを設定…」で開いたときに、そのまま打ち
            // はじめられるようにする。開く道が 1 つなので、奪う相手もいない。
            autoFocus
            // 1 行の欄なので Enter で確定する（`docs/DESIGN.md`）。
            onKeyDown={(event) => {
              if (event.key !== "Enter" || isComposing(event.nativeEvent)) return;
              event.preventDefault();
              void add();
            }}
            onChange={(event) => {
              setTitle(event.target.value);
            }}
          />
          <button
            type="button"
            className="primary add-recurrence"
            disabled={title.trim() === ""}
            onClick={() => void add()}
          >
            追加
          </button>
        </div>
        <p className="field-note">
          追加したものは毎日の繰り返しとして始まります。周期と入れ先は、この一覧で変えられます。
        </p>
        {failed?.field === "recurrenceTitle" && (
          <p className="field-error" role="alert">
            {failed.detail}
          </p>
        )}
      </div>
    </aside>
  );
}

/// 定義 1 つぶんの欄。**触った時点で確定します**（保存ボタンを置きません）。
///
/// 題と説明だけは `Enter` と焦点が外れたときに確定します。打っている途中の
/// 1 文字ごとに保存すると、往復のたびに盤面が差し替わります。
function RecurrenceRow({
  recurrence,
  columns,
  tags,
  run,
}: {
  recurrence: Recurrence;
  columns: readonly Column[];
  tags: readonly Tag[];
  run: Run;
}) {
  const [title, setTitle] = useState(recurrence.title);
  const [description, setDescription] = useState(recurrence.description);
  const [checklist, setChecklist] = useState(recurrence.checklist.join("\n"));
  const [failed, setFailed] = useState<AppError | null>(null);

  /// いまの欄から下書きを組み立て、渡された分だけ差し替える。
  function draftWith(changes: Partial<RecurrenceDraft>): RecurrenceDraft {
    return {
      title,
      description,
      columnId: recurrence.columnId,
      tagIds: recurrence.tagIds,
      // 1 行 1 項目。**空行は落とします**（モデル側でも落ちます）。
      checklist: checklist.split("\n").map((line) => line.trim()),
      schedule: recurrence.schedule,
      leadDays: recurrence.leadDays,
      previous: recurrence.previous,
      enabled: recurrence.enabled,
      ...changes,
    };
  }

  async function commit(changes: Partial<RecurrenceDraft>) {
    setFailed(await run((document) => updateRecurrence(document, recurrence.id, draftWith(changes))));
  }

  /// 打った名前のタグをその場で作り、この定義に付ける（#115、[ADR 0027]）。
  ///
  /// **作るのと付けるのを 1 回の `run()` にまとめます**——カードの編集パネルは
  /// 下書きを持っているので分けられますが、ここは触った時点で確定する作りなので、
  /// 分けると作っただけで付いていない状態が置き場所に残ります。色は渡しません
  /// ——決めていないタグには自動で色が付きます（ADR 0044）。
  ///
  /// [ADR 0027]: ../../../docs/adr/0027-creating-tags-while-editing-a-card.md
  async function createTag(name: string) {
    setFailed(
      await run((document) => {
        const created = addTag(document, name, AUTO_TAG_COLOR);
        if (!created.ok) return created;
        return updateRecurrence(
          document,
          recurrence.id,
          draftWith({ tagIds: [...recurrence.tagIds, created.value] }),
        );
      }),
    );
  }

  const { schedule } = recurrence;
  const lead = leadDaysOf(schedule, recurrence.leadDays);
  const leadable = schedule.kind !== "daily" && schedule.kind !== "weekday";

  return (
    <section className="recurrence-row" data-recurrence={recurrence.id}>
      <div className="recurrence-head">
        <input
          className="field-input recurrence-title-input"
          value={title}
          aria-label={`${recurrence.title} の題`}
          onChange={(event) => {
            setTitle(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key !== "Enter" || isComposing(event.nativeEvent)) return;
            event.preventDefault();
            void commit({});
          }}
          // 焦点が外れたときにも確定する。打ってから別の欄へ移った操作を、
          // 打たなかったことにしない（タグ整理パネルと同じ）。
          onBlur={() => void commit({})}
        />
        {/* 色だけに意味を持たせないので、止まっていることは文言でも出す
            （`docs/DESIGN.md`「画面の作り」）。 */}
        <label className="recurrence-enabled">
          <input
            type="checkbox"
            checked={recurrence.enabled}
            aria-label={`${recurrence.title} を有効にする`}
            onChange={(event) => {
              void commit({ enabled: event.target.checked });
            }}
          />
          有効
        </label>
        {/* 定義を消しても盤面のカードは残るので、確認は出しません
            （`docs/DESIGN.md`）。Undo で戻せます。 */}
        <button
          type="button"
          className="danger-item remove-recurrence"
          aria-label={`${recurrence.title} を削除`}
          onClick={() => {
            void run((document) => removeRecurrence(document, recurrence.id));
          }}
        >
          削除
        </button>
      </div>

      <div className="recurrence-fields">
        <label className="recurrence-field">
          <span className="field-label">周期</span>
          <select
            className="field-input"
            value={schedule.kind}
            aria-label={`${recurrence.title} の周期`}
            onChange={(event) => {
              void commit({ schedule: scheduleOfKind(event.target.value, schedule) });
            }}
          >
            {KINDS.map((each) => (
              <option key={each.kind} value={each.kind}>
                {each.label}
              </option>
            ))}
          </select>
        </label>

        {schedule.kind === "weekly" && (
          <div className="recurrence-field">
            <span className="field-label">曜日</span>
            <div className="recurrence-weekdays">
              {WEEKDAYS.map((name, day) => {
                const on = schedule.days.includes(day);
                return (
                  <button
                    key={name}
                    type="button"
                    className="secondary recurrence-weekday"
                    aria-pressed={on}
                    aria-label={`${recurrence.title} の${name}曜日`}
                    onClick={() => {
                      // **1 つも選ばれていない状態は作りません**——置き場所が
                      // 「曜日を 1 つも言っていない」として断ります。
                      const days = on
                        ? schedule.days.filter((each) => each !== day)
                        : [...schedule.days, day];
                      if (days.length === 0) return;
                      void commit({ schedule: { kind: "weekly", days: days.sort((a, b) => a - b) } });
                    }}
                  >
                    {on ? "✓ " : ""}
                    {name}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {schedule.kind === "monthly" && (
          <label className="recurrence-field">
            <span className="field-label">日</span>
            <input
              className="field-input recurrence-day-input"
              type="number"
              min={1}
              max={31}
              value={schedule.day}
              aria-label={`${recurrence.title} の日`}
              onChange={(event) => {
                const day = Number(event.target.value);
                if (!Number.isInteger(day) || day < 1 || day > 31) return;
                void commit({ schedule: { kind: "monthly", day } });
              }}
            />
          </label>
        )}

        <label className="recurrence-field">
          <span className="field-label">入れ先</span>
          <select
            className="field-input"
            value={recurrence.columnId}
            aria-label={`${recurrence.title} の入れ先`}
            onChange={(event) => {
              void commit({ columnId: Number(event.target.value) });
            }}
          >
            {/* 消えたカラムを指していることがあります。**選択肢に無い値**に
                なるので、そのことが読めるように 1 つ足します。 */}
            {!columns.some((column) => column.id === recurrence.columnId) && (
              <option value={recurrence.columnId}>（消えたカラム／一番左に入ります）</option>
            )}
            {columns.map((column) => (
              <option key={column.id} value={column.id}>
                {column.name}
              </option>
            ))}
          </select>
        </label>

        {/* 毎日と平日は先読みを持ちません（#198）。**欄は消さずに灰色にします**
            ——消すと周期を変えたときに行が動き、そこに何があったのかも読めなく
            なります。灰色は欄だけでなく見出しと注記にも当てて、区画ごと止まって
            いることが見て取れるようにし、理由は文言でも出します（`docs/DESIGN.md`
            「色だけに意味を持たせない」）。 */}
        <label className={`recurrence-field${leadable ? "" : " is-disabled"}`}>
          <span className="field-label">先読み</span>
          <input
            className="field-input recurrence-lead-input"
            type="number"
            min={0}
            max={60}
            value={lead}
            disabled={!leadable}
            aria-label={`${recurrence.title} の先読み日数`}
            onChange={(event) => {
              const days = Number(event.target.value);
              if (!Number.isInteger(days) || days < 0 || days > 60) return;
              void commit({ leadDays: days });
            }}
          />
          <span className="field-note">
            {leadable ? "日前から出す" : "毎日・平日は先読みを持ちません"}
          </span>
        </label>

        <label className="recurrence-field">
          <span className="field-label">前回のカード</span>
          <select
            className="field-input"
            value={recurrence.previous}
            aria-label={`${recurrence.title} の前回のカード`}
            onChange={(event) => {
              void commit({ previous: event.target.value as PreviousPolicy });
            }}
          >
            {POLICIES.map((policy) => (
              <option key={policy.value} value={policy.value}>
                {policy.label}
              </option>
            ))}
          </select>
        </label>
      </div>

      <label className="recurrence-field">
        <span className="field-label">説明</span>
        <textarea
          className="field-input recurrence-description-input"
          value={description}
          rows={2}
          placeholder="カードの説明（任意）"
          aria-label={`${recurrence.title} の説明`}
          onChange={(event) => {
            setDescription(event.target.value);
          }}
          onBlur={() => void commit({})}
        />
      </label>

      <label className="recurrence-field">
        <span className="field-label">チェックリスト</span>
        <textarea
          className="field-input recurrence-checklist-input"
          value={checklist}
          rows={2}
          placeholder="1 行に 1 項目（任意）"
          aria-label={`${recurrence.title} のチェックリスト`}
          onChange={(event) => {
            setChecklist(event.target.value);
          }}
          onBlur={() => void commit({})}
        />
      </label>

      {/* カードの編集パネルと**同じ欄**です（`panel/TagsInput.tsx`）——付ける
          ものが同じなら、打ち方も同じにします。無いタグはここで作れます。 */}
      <div className="recurrence-field">
        <span className="field-label">タグ</span>
        <TagsInput
          tags={tags}
          selected={recurrence.tagIds}
          failure={failed}
          // 定義の数だけ並ぶ欄なので、読み上げでどれのタグか分かるようにする。
          context={recurrence.title}
          onToggle={(tagId) => {
            const tagIds = recurrence.tagIds.includes(tagId)
              ? recurrence.tagIds.filter((id) => id !== tagId)
              : [...recurrence.tagIds, tagId];
            void commit({ tagIds });
          }}
          onCreate={createTag}
        />
      </div>

      <p className="field-note recurrence-summary">
        {describeSchedule(schedule)}
        {lead > 0 && `（${String(lead)} 日前に出す）`}
        {recurrence.lastGeneratedOn === null
          ? " · まだ出していません"
          : ` · 最後に出したのは ${recurrence.lastGeneratedOn} のぶん`}
      </p>
      {failed?.field === "recurrenceTitle" && (
        <p className="field-error" role="alert">
          {failed.detail}
        </p>
      )}
    </section>
  );
}

/// 選ばれた種類の周期を組み立てる。**前に選んでいた値は引き継ぎません**——
/// 「毎週」に戻したときの曜日は、そのつど選び直すほうが読み違えません。
function scheduleOfKind(kind: string, current: Schedule): Schedule {
  switch (kind) {
    case "weekday":
      return { kind: "weekday" };
    case "weekly":
      return current.kind === "weekly" ? current : { kind: "weekly", days: [0] };
    case "monthly":
      return current.kind === "monthly" ? current : { kind: "monthly", day: 1 };
    case "monthlyLast":
      return { kind: "monthlyLast" };
    default:
      return { kind: "daily" };
  }
}
