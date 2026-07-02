import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";

export default defineConfig({
  define: {
    "process.env.NODE_ENV": JSON.stringify(
      process.env.NODE_ENV || "production"
    ),
  },
  plugins: [
    vue(),
    {
      name: "aiohub-alias-resolver",
      enforce: "pre",
      resolveId(source) {
        if (source.startsWith("@/")) {
          const isUI =
            source.includes("/components/") || source.includes("/tools/");
          return { id: isUI ? "aiohub-ui" : "aiohub-sdk", external: true };
        }
        return null;
      },
    },
  ],
  resolve: {
    alias: {
      "@": resolve(__dirname, "../../src"),
      "aiohub-sdk": resolve(__dirname, "../../src/services/plugin-sdk"),
      "aiohub-ui": resolve(__dirname, "../../src/services/plugin-ui"),
    },
  },
  build: {
    lib: {
      entry: resolve(__dirname, "PaddleOcr.vue"),
      name: "PaddleOcr",
      fileName: "PaddleOcr",
      formats: ["es"],
    },
    rollupOptions: {
      external: [
        "vue",
        "@tauri-apps/api/core",
        "@tauri-apps/api/path",
        "@tauri-apps/plugin-fs",
        "aiohub-sdk",
        "aiohub-ui",
      ],
      output: {
        codeSplitting: false, // Vite 8 / Rolldown 推荐写法：禁用代码分割，消灭分块 JS，彻底解决相对路径加载问题
        globals: {
          vue: "Vue",
        },
      },
    },
    outDir: "dist",
    emptyOutDir: false,
  },
});
