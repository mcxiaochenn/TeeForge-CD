import { createApp } from 'vue';

import './styles/tailwind.css';
import App from './App.vue';
import { router } from './router';
import { initI18n } from './lib/i18n';
import { loadSession } from './state/session';

// 字典加载完成后再挂载，避免首帧显示键名。
initI18n().finally(() => {
  const app = createApp(App);
  // 渲染异常时留下可读标记，避免整页空白没有任何线索。
  app.config.errorHandler = (error) => {
    const message = error instanceof Error ? error.message : String(error);
    console.error(message);
    document.getElementById('app')?.setAttribute('data-render-error', message);
  };
  app.use(router).mount('#app');
  void loadSession();
});
