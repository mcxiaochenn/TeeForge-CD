/** 会话状态：首屏数据只取一次，页面之间共享 [Shared session state] */

import { reactive } from 'vue';
import { api, type Bootstrap } from '../lib/api';

export const session = reactive({
  loading: false,
  loaded: false,
  error: '',
  data: null as Bootstrap | null,
});

let inflight: Promise<void> | null = null;

export async function loadSession(force = false): Promise<void> {
  if (session.loading && inflight) return inflight;
  if (session.loaded && !force) return;
  session.loading = true;
  session.error = '';
  inflight = (async () => {
    try {
      session.data = await api.bootstrap();
      session.loaded = true;
    } catch (error) {
      session.error = error instanceof Error ? error.message : String(error);
      session.loaded = true;
    } finally {
      session.loading = false;
      inflight = null;
    }
  })();
  return inflight;
}

export function refreshSession(): Promise<void> {
  return loadSession(true);
}
