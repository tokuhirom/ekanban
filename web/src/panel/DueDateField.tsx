// 期限の欄（#194、[ADR 0046]）。
//
// **選ぶ道と打つ道の両方を出します。** カレンダーから日を押しても、`9/12`
// `明日` `金` `+3` と打っても、入るのは同じ 1 つの欄です。カレンダーは値を
// 持ちません——同じ値を触る欄が 2 つあると、どちらが本物かを決める規則が要ります。
//
// 読み方は `model/due.ts` の 1 つだけで、ここでは数えません（ADR 0031）。
// カレンダーの升目は `panel/calendar.ts`、日付の足し算は `model/dates.ts` です。
//
// [ADR 0046]: ../../../docs/adr/0046-picking-a-due-date-from-a-calendar.md

import { useEffect, useMemo, useRef, useState } from "react";

import { dueChoices } from "../board/due";
import { addDays, formatIsoDate, parseIsoDate } from "../model/dates";
import { parseDueDate } from "../model/due";
import { isComposing } from "../shell/ime";
import {
  CALENDAR_WEEKDAYS,
  dayLabel,
  monthLabel,
  monthOf,
  monthWeeks,
  shiftMonth,
} from "./calendar";

/** 矢印キーで動く日数。カレンダーの升目の並びと同じ。 */
const ARROW_DAYS: Record<string, number> = {
  ArrowLeft: -1,
  ArrowRight: 1,
  ArrowUp: -7,
  ArrowDown: 7,
};

interface Props {
  /** 欄に入っている文字。`"YYYY-MM-DD"` とは限らない（打っている途中も来る）。 */
  value: string;
  /** 期限を読むときの基準日（`"YYYY-MM-DD"`）。手元の時計をここで読まない。 */
  today: string;
  /** 打っている途中。**確定しない。** */
  onType: (value: string) => void;
  /** 欄を離れた・`Enter`。確定するかどうかは呼ぶ側が決める。 */
  onCommit: () => void;
  /** カレンダーか近道で選んだ。**その場で確定する**（空文字は期限なし）。 */
  onPick: (value: string) => void;
}

export function DueDateField({ value, today, onType, onCommit, onPick }: Props) {
  const [open, setOpen] = useState(false);
  // ← → で送った月（`"YYYY-MM"`）。**どの月から送ったかも一緒に覚えます**
  // ——欄に打たれた日付が変われば、送った先ではなくその月を出したいので。
  const [shifted, setShifted] = useState<{ from: string; month: string } | null>(null);
  // 矢印キーで動かしている日。押して確かめる前の、いわば影の選択です。
  const [cursor, setCursor] = useState<string | null>(null);
  const field = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const grid = useRef<HTMLDivElement>(null);
  // 選んだあとに欄へ焦点を戻すとき、その焦点でカレンダーを開き直さないための札。
  const reopen = useRef(true);

  // 欄の文字がどの日を指しているか。読めない間は `null`（カレンダーは今日の月）。
  const selected = useMemo(() => {
    const read = parseDueDate(value, today);
    return read.ok ? read.date : null;
  }, [value, today]);

  const anchor = monthOf(selected ?? today) ?? "";
  const month =
    cursor !== null
      ? (monthOf(cursor) ?? anchor)
      : shifted !== null && shifted.from === anchor
        ? shifted.month
        : anchor;
  const weeks = useMemo(() => monthWeeks(month), [month]);

  // タブで升目に入ったときに当たる日。出している月に無ければ、その月の 1 日。
  const days = weeks.flat();
  const wanted = cursor ?? selected ?? today;
  const focusDate = days.some((day) => day.date === wanted)
    ? wanted
    : (days.find((day) => day.inMonth)?.date ?? null);

  // 矢印キーで動かした先へ焦点を移す。日付は `"YYYY-MM-DD"` に限られるので、
  // ここで組み立てるセレクタに打った文字は入りません。
  useEffect(() => {
    if (cursor === null) return;
    grid.current?.querySelector<HTMLButtonElement>(`[data-date="${cursor}"]`)?.focus();
  }, [cursor]);

  // 開いている間だけ、外を押したときに畳む（メニューバーと同じ出し方）。
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent): void => {
      if (!(event.target instanceof Node)) return;
      if (field.current?.contains(event.target) === true) return;
      // 畳むだけ。焦点は押した先へ移るので、欄へは戻しません。
      setOpen(false);
      setShifted(null);
      setCursor(null);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
    };
  }, [open]);

  function close(): void {
    setOpen(false);
    setShifted(null);
    setCursor(null);
  }

  /// カレンダーを畳んで、欄へ焦点を戻す。
  ///
  /// 戻した焦点で開き直さないよう、札を降ろしてから戻します。
  function closeToInput(): void {
    close();
    reopen.current = false;
    input.current?.focus();
    reopen.current = true;
  }

  /// 選ばれた日をそのまま欄へ入れて確定する。空文字は期限なし。
  function pick(date: string): void {
    onPick(date);
    closeToInput();
  }

  return (
    <div className="due-field" ref={field}>
      <input
        id="card-due-date"
        ref={input}
        type="text"
        className="field-input card-due-input"
        aria-label="期限"
        // 何が打てるかは placeholder が言います。選ぶだけなら読まずに済みます。
        placeholder="期限（9/12、明日、金、+3）"
        autoComplete="off"
        value={value}
        onChange={(event) => {
          // 打ち直されたら、送った月も矢印の位置も捨てます。打った日付の月が
          // 出ていないと、読み違いに気づけません。
          setShifted(null);
          setCursor(null);
          onType(event.target.value);
        }}
        // **触れば開きます**（#194）。押さないと出てこないカレンダーは、無いのと
        // 同じくらい気づけません。焦点は欄に残るので、そのまま打てます。
        onFocus={() => {
          if (reopen.current) setOpen(true);
        }}
        onBlur={onCommit}
        // 1 行の欄なので `Enter` で確定（`docs/DESIGN.md`）。
        onKeyDown={(event) => {
          if (isComposing(event.nativeEvent)) return;
          if (event.key === "Enter") {
            event.preventDefault();
            close();
            onCommit();
            return;
          }
          // **`Escape` はカレンダーだけを畳みます。** パネルまで届くと、
          // カレンダーを閉じたつもりでパネルごと閉じます。
          if (event.key === "Escape" && open) {
            event.stopPropagation();
            closeToInput();
          }
        }}
      />
      {/* 外す × は欄に重ねず右へ並べます。期限が入っているときだけ出すので、
          期限なしのカードでは欄が入力とカレンダーだけになります（#128）。 */}
      {value !== "" && (
        <button
          type="button"
          className="ghost due-clear"
          aria-label="期限を外す"
          title="期限を外す"
          onClick={() => {
            pick("");
          }}
        >
          ×
        </button>
      )}
      <button
        type="button"
        className="ghost due-calendar-button"
        aria-label="カレンダーから選ぶ"
        title="カレンダーから選ぶ"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => {
          if (open) closeToInput();
          else setOpen(true);
        }}
      >
        📅
      </button>
      {open && (
        <div
          className="menu due-calendar"
          role="dialog"
          aria-label="期限のカレンダー"
          onKeyDown={(event) => {
            if (event.key !== "Escape" || isComposing(event.nativeEvent)) return;
            event.stopPropagation();
            closeToInput();
          }}
        >
          <div className="due-calendar-head">
            <button
              type="button"
              className="ghost"
              aria-label="前の月"
              onClick={() => {
                const previous = shiftMonth(month, -1);
                if (previous !== null) setShifted({ from: anchor, month: previous });
                setCursor(null);
              }}
            >
              ‹
            </button>
            <span className="due-calendar-month" aria-live="polite">
              {monthLabel(month)}
            </span>
            <button
              type="button"
              className="ghost"
              aria-label="次の月"
              onClick={() => {
                const next = shiftMonth(month, 1);
                if (next !== null) setShifted({ from: anchor, month: next });
                setCursor(null);
              }}
            >
              ›
            </button>
          </div>
          {/* 矢印キーでも動かせます（#194）。升目の並びどおり、← → で 1 日、
              ↑ ↓ で 1 週。月をまたぐと、出す月のほうが追いかけます。 */}
          <div
            className="due-calendar-grid"
            ref={grid}
            onKeyDown={(event) => {
              const step = ARROW_DAYS[event.key];
              if (step === undefined || isComposing(event.nativeEvent)) return;
              event.preventDefault();
              const base = parseIsoDate(focusDate ?? today);
              if (base === null) return;
              const moved = formatIsoDate(addDays(base, step));
              if (moved !== null) setCursor(moved);
            }}
          >
            <div className="due-calendar-week due-calendar-heads">
              {CALENDAR_WEEKDAYS.map((weekday) => (
                <span key={weekday} className="due-calendar-weekday">
                  {weekday}
                </span>
              ))}
            </div>
            {weeks.map((week) => (
              <div key={week[0]?.date} className="due-calendar-week">
                {week.map((day) => (
                  <button
                    key={day.date}
                    type="button"
                    className="ghost due-calendar-day"
                    data-date={day.date}
                    // 月外の日と今日は、色で描き分けます（`styles.css`）。
                    data-outside={day.inMonth ? undefined : true}
                    data-today={day.date === today ? true : undefined}
                    aria-label={dayLabel(day.date)}
                    aria-pressed={day.date === selected}
                    // 升目に入る道は 1 つ。中は矢印キーで動きます。
                    tabIndex={day.date === focusDate ? 0 : -1}
                    onClick={() => {
                      pick(day.date);
                    }}
                  >
                    {day.day}
                  </button>
                ))}
              </div>
            ))}
          </div>
          {/* 近道は右クリックメニューと同じ候補から採ります（`board/due.ts`）
              ——同じ言葉が 2 か所で違う日を指さないように。 */}
          <div className="due-calendar-choices">
            {dueChoices(today).map((choice) => (
              <button
                key={choice.label}
                type="button"
                className="ghost"
                onClick={() => {
                  pick(choice.date);
                }}
              >
                {choice.label}
              </button>
            ))}
            <button
              type="button"
              className="ghost"
              onClick={() => {
                pick("");
              }}
            >
              なし
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
