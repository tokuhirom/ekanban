// 「日付が変わったか」だけを見るための小さな部品（#135）。
//
// **期限そのものはここで判定しません。** 期限が近いか過ぎたかを決めるのは
// `model/due.ts` で、基準日は画面が 1 つだけ持ちます（`BoardState.today`、
// `docs/DESIGN.md`「絞り込みと検索」）。ここが時計から作るのは比べるための
// 1 文字列だけで、違っていたら基準日を進めます。

/// 手元の時計の日付を `"YYYY-MM-DD"` で。
///
/// **地方時で数えます。** `toISOString()` は UTC なので、時差のある場所では
/// 日付が 1 日ずれ、日付が変わっていないのに変わったと見なします。
export function localDay(now: Date): string {
  const year = String(now.getFullYear()).padStart(4, "0");
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

/// スナップショットが出た日から、手元の日付が変わっているか。
export function dayHasTurned(today: string, now: Date): boolean {
  return localDay(now) !== today;
}
