// 「日付が変わったか」だけを見るための小さな部品（#135）。
//
// **手元の時計で期限を判定しません。** 期限が近いか過ぎたかを決めるのは Rust の
// `due_status` で、その結果と基準日（`Snapshot.today`）はスナップショットに
// 載って来ます（`docs/DESIGN.md`「絞り込みと検索」）。ここが時計から作るのは
// 比べるための 1 文字列だけで、違っていたら Rust に聞き直します。

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
