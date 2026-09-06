import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // 画面を組み立てるテストはここではやりません（§10）。DOM は要らない。
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
