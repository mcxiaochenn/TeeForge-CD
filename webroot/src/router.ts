import { createRouter, createWebHashHistory } from 'vue-router';

import HomePage from './pages/HomePage.vue';
import KeyboxPage from './pages/KeyboxPage.vue';
import RkpPage from './pages/RkpPage.vue';
import TargetPage from './pages/TargetPage.vue';
import UpdatePage from './pages/UpdatePage.vue';

// hash 路由：管理器从固定入口加载 index.html，不需要服务端重写。
export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: HomePage, meta: { titleKey: 'app.title' } },
    { path: '/target', name: 'target', component: TargetPage, meta: { titleKey: 'entry.target.title' } },
    { path: '/keybox', name: 'keybox', component: KeyboxPage, meta: { titleKey: 'entry.keybox.title' } },
    { path: '/rkp', name: 'rkp', component: RkpPage, meta: { titleKey: 'entry.rkp.title' } },
    { path: '/update', name: 'update', component: UpdatePage, meta: { titleKey: 'entry.update.title' } },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
  scrollBehavior: () => ({ top: 0 }),
});
