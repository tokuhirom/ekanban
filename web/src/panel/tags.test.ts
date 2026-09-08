// タグの上の純粋な操作のテスト（`docs/DESIGN.md`「テスト」の「部品」）。

import { describe, expect, it } from "vitest";

import type { Tag } from "../ipc/types/Tag";
import {
  findTagByName,
  suggestTags,
  TAG_PALETTE,
  tagChipStyle,
  tagColor,
  tagTextColor,
} from "./tags";

function tag(id: number, name: string, color = ""): Tag {
  return { id, boardId: 1, name, color, createdAt: 0, updatedAt: 0 };
}

const tags: Tag[] = [tag(1, "仕事"), tag(2, "Rust"), tag(3, "買い物")];

describe("findTagByName", () => {
  it("前後の空白と大文字小文字を無視して引く", () => {
    expect(findTagByName(tags, "  rust ")?.id).toBe(2);
    expect(findTagByName(tags, "仕事")?.id).toBe(1);
  });

  it("無ければ null", () => {
    expect(findTagByName(tags, "家事")).toBeNull();
  });

  it("空白だけの名前は何にも当てない", () => {
    expect(findTagByName(tags, "   ")).toBeNull();
  });
});

describe("suggestTags", () => {
  it("既に選んであるタグは候補に出さない", () => {
    expect(suggestTags(tags, [1], "仕").map((tag) => tag.id)).toEqual([]);
    expect(suggestTags(tags, [1], "s").map((tag) => tag.id)).toEqual([2]);
  });

  it("打ちかけの文字で絞る。大文字小文字は同じものとして扱う", () => {
    expect(suggestTags(tags, [], "RU").map((tag) => tag.id)).toEqual([2]);
    expect(suggestTags(tags, [], "買").map((tag) => tag.id)).toEqual([3]);
  });

  it("何も打っていなければ、1 つも出さない", () => {
    expect(suggestTags(tags, [], "")).toEqual([]);
    expect(suggestTags(tags, [], "   ")).toEqual([]);
  });
});

describe("tagColor", () => {
  it("色を決めていないタグには、ID の順にパレットの色が付く", () => {
    expect(tagColor(tag(1, "仕事"))).toBe(TAG_PALETTE[0]);
    expect(tagColor(tag(2, "Rust"))).toBe(TAG_PALETTE[1]);
    expect(tagColor(tag(3, "買い物"))).toBe(TAG_PALETTE[2]);
  });

  it("続けて作ったタグは、隣り合っても違う色になる", () => {
    const colors = [1, 2, 3, 4, 5].map((id) => tagColor(tag(id, `${id}`)));
    expect(new Set(colors).size).toBe(colors.length);
  });

  it("パレットより多く作ると一巡する", () => {
    expect(tagColor(tag(TAG_PALETTE.length + 1, "一巡"))).toBe(TAG_PALETTE[0]);
  });

  it("ユーザーが決めた色は、そのまま使う", () => {
    expect(tagColor(tag(1, "仕事", "#123456"))).toBe("#123456");
  });
});

describe("tagTextColor", () => {
  it("明るい色には濃い文字、暗い色には淡い文字を載せる", () => {
    expect(tagTextColor("#ffffff")).toBe("#0f172a");
    expect(tagTextColor("#eab308")).toBe("#0f172a");
    expect(tagTextColor("#000000")).toBe("#f8fafc");
    expect(tagTextColor("#6366f1")).toBe("#f8fafc");
  });

  // 明るさで選んだ文字色が、実際に読めることまで見る。4.5:1 は WCAG AA が
  // 普通の大きさの文字に求める比で、チップの 12px はそれに当たる。
  it("パレットのどの色でも、文字が 4.5:1 以上の差で載る", () => {
    for (const color of TAG_PALETTE) {
      expect(contrast(color, tagTextColor(color))).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("読めない色でも落ちない", () => {
    expect(tagTextColor("")).toBe("#0f172a");
    expect(tagTextColor("あか")).toBe("#0f172a");
  });

  it("3 桁の指定も読む", () => {
    expect(tagTextColor("#fff")).toBe("#0f172a");
    expect(tagTextColor("#000")).toBe("#f8fafc");
  });
});

describe("tagChipStyle", () => {
  it("背景と、その上で読める文字色を返す", () => {
    expect(tagChipStyle(tag(1, "仕事"))).toEqual({
      background: TAG_PALETTE[0],
      color: tagTextColor(TAG_PALETTE[0]),
    });
  });
});

/// テストの中だけで使う、WCAG のコントラスト比。実装は明るさの境目だけを
/// 決めていて、比そのものは数えていないので、ここで独立に数える。
function contrast(a: string, b: string): number {
  const bright = Math.max(luminance(a), luminance(b));
  const dark = Math.min(luminance(a), luminance(b));
  return (bright + 0.05) / (dark + 0.05);
}

function luminance(color: string): number {
  const value = Number.parseInt(color.replace("#", ""), 16);
  const channels = [(value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff]
    .map((channel) => channel / 255)
    .map((channel) =>
      channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
    );
  return (
    0.2126 * (channels[0] ?? 0) + 0.7152 * (channels[1] ?? 0) + 0.0722 * (channels[2] ?? 0)
  );
}
