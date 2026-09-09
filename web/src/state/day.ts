// 「日付が変わったか」だけを見るための小さな部品（#135、#197）。
//
// **期限そのものはここで判定しません。** 期限が近いか過ぎたかを決めるのは
// `model/due.ts` で、基準日は画面が 1 つだけ持ちます（`BoardState.today`、
// `docs/DESIGN.md`「絞り込みと検索」）。ここが時計から作るのは比べるための
// 1 文字列だけで、違っていたら基準日を進めます。
//
// **日は 0 時に変わるとは限りません**（[ADR 0048]）。境界時刻 `h` の日 D は
// 「D の h 時から D+1 の h 時まで」で、既定は午前 4 時です。境界は呼ぶ側が
// 渡します——ここが設定を読みに行くと、時計を読まない形（`now` が引数）と
// 揃わなくなります。
//
// [ADR 0048]: ../../../docs/adr/0048-the-day-turns-at-four-in-the-morning.md

/// 日付が変わる時刻の既定（[ADR 0048]）。
///
/// [ADR 0048]: ../../../docs/adr/0048-the-day-turns-at-four-in-the-morning.md
export const DEFAULT_DAY_BOUNDARY_HOUR = 4;

/// 置き場所から読んだ境界時刻を、0〜23 の整数にする。
///
/// 読めない値は既定に落とします。**画面を止めるほどのことではありません**
/// ——境界時刻が壊れていても、盤面は開けます。
export function readDayBoundaryHour(stored: string | number | null | undefined): number {
  // **空文字と `null` を `Number()` に渡しません。** どちらも 0 になるので、
  // 何も置かれていない置き場所が「0 時境界を選んだ」ことになってしまいます。
  if (stored === null || stored === undefined || stored === "") return DEFAULT_DAY_BOUNDARY_HOUR;
  const hour = typeof stored === "number" ? stored : Number(stored);
  if (!Number.isInteger(hour) || hour < 0 || hour > 23) return DEFAULT_DAY_BOUNDARY_HOUR;
  return hour;
}

/// 手元の時計から見た基準日を `"YYYY-MM-DD"` で。
///
/// **地方時で数えます。** `toISOString()` は UTC なので、時差のある場所では
/// 日付が 1 日ずれ、日付が変わっていないのに変わったと見なします。
///
/// `boundaryHour` より前の時刻は、前日を返します。既定の 4 なら、午前 3 時は
/// まだ前日です。
export function localDay(now: Date, boundaryHour: number): string {
  // その日の正午を起点に日付だけを動かします。**時間を引きません**——夏時間の
  // 切り替わる日には 1 日が 23 時間にも 25 時間にもなるので、時刻の引き算では
  // 日付が戻りすぎたり戻らなかったりします。
  const base = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 12);
  if (now.getHours() < boundaryHour) base.setDate(base.getDate() - 1);
  const year = String(base.getFullYear()).padStart(4, "0");
  const month = String(base.getMonth() + 1).padStart(2, "0");
  const day = String(base.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/// スナップショットが出た日から、手元の基準日が変わっているか。
export function dayHasTurned(today: string, now: Date, boundaryHour: number): boolean {
  return localDay(now, boundaryHour) !== today;
}
