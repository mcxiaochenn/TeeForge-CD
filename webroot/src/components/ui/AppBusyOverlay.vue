<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';

import { t } from '../../lib/i18n';

const props = withDefaults(defineProps<{ open: boolean; label?: string }>(), { label: '' });

/**
 * 等待浮层 [Busy overlay]
 *
 * 只在命令确实要跑一会儿时出现：350ms 内结束的操作不弹，避免闪一下反而更像卡顿。
 * 计时器每秒递增，是"还活着"的最直接证据；纯转圈看不出新进展。
 * The elapsed counter is the actual liveness signal; a bare spinner is not.
 */
const SHOW_DELAY = 350;

const visible = ref(false);
const seconds = ref(0);
let showTimer: number | undefined;
let tickTimer: number | undefined;

function clearTimers() {
  if (showTimer !== undefined) window.clearTimeout(showTimer);
  if (tickTimer !== undefined) window.clearInterval(tickTimer);
  showTimer = undefined;
  tickTimer = undefined;
}

watch(
  () => props.open,
  (open) => {
    clearTimers();
    if (!open) {
      visible.value = false;
      return;
    }
    seconds.value = 0;
    showTimer = window.setTimeout(() => {
      visible.value = true;
      tickTimer = window.setInterval(() => {
        seconds.value += 1;
      }, 1000);
    }, SHOW_DELAY);
  },
  { immediate: true },
);

onBeforeUnmount(clearTimers);
</script>

<template>
  <Teleport to="body">
    <transition name="busy">
      <div
        v-if="visible"
        class="fixed inset-0 z-[60] flex items-center justify-center bg-black/30 px-6"
        role="status"
        aria-live="polite"
      >
        <div class="w-full max-w-[300px] rounded-card bg-surface px-6 py-6 text-center shadow-sheet">
          <span
            class="mx-auto block h-9 w-9 animate-spin rounded-full border-[3px] border-accent-soft border-t-accent"
            aria-hidden="true"
          />
          <p class="mt-4 text-sm font-semibold leading-relaxed">
            {{ props.label || t('common.working') }}
          </p>
          <p class="mt-1 font-mono text-[11px] text-faint">
            {{ t('common.busy.elapsed', { n: seconds }) }}
          </p>
          <p class="mt-3 text-[11px] leading-relaxed text-muted">{{ t('common.busy.hint') }}</p>
        </div>
      </div>
    </transition>
  </Teleport>
</template>
