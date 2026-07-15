// @ts-check
import { defineConfig } from 'astro/config';

// https://astro.build/config
export default defineConfig({
  outDir: '../module/webroot',
  build: {
    format: 'file',
  },
  vite: {
    build: {
      cssMinify: true,
    },
  },
});
