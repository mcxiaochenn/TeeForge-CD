import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

// 输出到模块的 webroot 目录，与 xtask 打包路径保持一致。
// Assets use relative paths so the page works from any mount point.
export default defineConfig({
  plugins: [vue()],
  base: './',
  build: {
    outDir: '../module/webroot',
    emptyOutDir: true,
    // 兼容较旧的 Android WebView（模块基线 API 24+）。
    target: 'chrome87',
    cssCodeSplit: false,
    assetsInlineLimit: 4096,
  },
});
