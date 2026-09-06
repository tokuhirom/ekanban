// タグの上の純粋な操作のテスト（`docs/DESIGN.md`「テスト」の「部品」）。

import { describe, expect, it } from "vitest";

import type { Tag } from "../ipc/types/Tag";
import { findTagByName, suggestTags } from "./tags";

function tag(id: number, name: string): Tag {
  return { id, boardId: 1, name, color: "#94a3b8", createdAt: 0, updatedAt: 0 };
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
    expect(suggestTags(tags, [1], "").map((tag) => tag.id)).toEqual([2, 3]);
  });

  it("打ちかけの文字で絞る。大文字小文字は同じものとして扱う", () => {
    expect(suggestTags(tags, [], "RU").map((tag) => tag.id)).toEqual([2]);
    expect(suggestTags(tags, [], "買").map((tag) => tag.id)).toEqual([3]);
  });

  it("何も打っていなければ、選んでいないものを全部出す", () => {
    expect(suggestTags(tags, [], "").map((tag) => tag.id)).toEqual([1, 2, 3]);
  });
});
