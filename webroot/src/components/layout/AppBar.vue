<script setup lang="ts">
import { ref } from 'vue';
import { useRouter } from 'vue-router';

import AppIcon from '../ui/AppIcon.vue';
import PreferencesSheet from './PreferencesSheet.vue';

const props = defineProps<{ title: string; showBack: boolean }>();
const router = useRouter();
const preferencesOpen = ref(false);

function goBack() {
  // 直接打开二级页时没有可回退的历史，回到首页而不是退出 WebUI。
  const state = window.history.state as { back?: string | null } | null;
  if (state?.back) router.back();
  else void router.push('/');
}
</script>

<template>
  <header
    class="sticky top-0 z-30 -mx-4 px-4 backdrop-blur-xl backdrop-saturate-150 safe-top"
    :style="{ background: 'var(--bar-bg)' }"
  >
    <div class="mx-auto flex h-14 w-full max-w-[560px] items-center gap-2">
      <button
        v-if="props.showBack"
        type="button"
        class="-ml-2 grid h-10 w-10 place-items-center rounded-full text-muted tap hover:bg-surface-alt hover:text-fg active:scale-95"
        aria-label="back"
        @click="goBack"
      >
        <AppIcon name="back" :size="20" />
      </button>
      <div v-else class="ml-1 grid h-9 w-9 place-items-center rounded-[11px] border border-accent-strong bg-accent-soft text-accent">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M12 2.9 20 7.5v9L12 21.1 4 16.5v-9z" />
          <path d="M12 7.8v3.6" />
          <circle cx="12" cy="14.7" r="1.5" fill="currentColor" stroke="none" />
        </svg>
      </div>

      <h1 class="min-w-0 flex-1 truncate px-1 text-[17px] font-semibold leading-6">
        {{ props.title }}
      </h1>

      <button
        type="button"
        class="-mr-2 grid h-10 w-10 place-items-center rounded-full text-muted tap hover:bg-surface-alt hover:text-fg active:scale-95"
        :aria-label="'preferences'"
        @click="preferencesOpen = true"
      >
        <AppIcon name="sliders" :size="20" />
      </button>
    </div>
  </header>

  <PreferencesSheet v-model:open="preferencesOpen" />
</template>
