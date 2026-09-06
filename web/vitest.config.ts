import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // 画面を組み立てるテストはここではやりません（`docs/DESIGN.md`「テスト」）。DOM は要らない。
    environment: "node",
    // e2e は Playwright の担当。Vitest には拾わせない。
    include: ["src/**/*.test.ts"],
  },
});
