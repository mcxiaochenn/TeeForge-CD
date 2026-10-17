<script setup lang="ts">
import { computed, ref, watch } from 'vue';

import AppSheet from '../ui/AppSheet.vue';
import SegmentedControl from '../ui/SegmentedControl.vue';
import { i18nState, setLanguage, t, type LangChoice } from '../../lib/i18n';

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (event: 'update:open', value: boolean): void }>();

function readMode(): string {
  try {
    const saved = JSON.parse(localStorage.getItem('tf-theme') || 'null');
    if (saved?.mode === 'light' || saved?.mode === 'dark') return saved.mode;
  } catch {
    /* ignore malformed value */
  }
  return document.documentElement.getAttribute('data-mode') || 'auto';
}

function readLang(): LangChoice {
  const saved = localStorage.getItem('tf-lang');
  return saved === 'en' || saved === 'zh' || saved === 'ja' ? saved : 'auto';
}

const mode = ref(readMode());
const lang = ref<LangChoice>(readLang());

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    mode.value = readMode();
    lang.value = readLang();
  },
);

function setMode(value: string) {
  mode.value = value;
  document.documentElement.setAttribute('data-mode', value);
  localStorage.setItem('tf-theme', JSON.stringify({ theme: 'material', mode: value }));
}

function setLang(value: string) {
  lang.value = value as LangChoice;
  void setLanguage(lang.value);
}

const appearanceOptions = computed(() => [
  { value: 'auto', label: t('prefs.auto') },
  { value: 'light', label: t('prefs.light') },
  { value: 'dark', label: t('prefs.dark') },
]);

const languageOptions = computed(() => [
  { value: 'auto', label: t('prefs.auto') },
  { value: 'zh', label: '中文' },
  { value: 'en', label: 'EN' },
  { value: 'ja', label: '日本語' },
]);

const detected = computed(() => i18nState.lang.toUpperCase());
</script>

<template>
  <AppSheet
    :open="props.open"
    :title="t('prefs.title')"
    :subtitle="t('prefs.subtitle')"
    @update:open="emit('update:open', $event)"
  >
    <div class="flex flex-col gap-5 pt-1">
      <div class="flex flex-col gap-2">
        <span class="text-[11px] font-bold uppercase tracking-[0.09em] text-muted">
          {{ t('prefs.appearance') }}
        </span>
        <SegmentedControl
          :model-value="mode"
          :options="appearanceOptions"
          :label="t('prefs.appearance')"
          @update:model-value="setMode"
        />
      </div>

      <div class="flex flex-col gap-2">
        <span class="text-[11px] font-bold uppercase tracking-[0.09em] text-muted">
          {{ t('prefs.language') }}
        </span>
        <SegmentedControl
          :model-value="lang"
          :options="languageOptions"
          :label="t('prefs.language')"
          @update:model-value="setLang"
        />
        <p class="text-[11px] text-faint">{{ t('prefs.detected', { lang: detected }) }}</p>
      </div>
    </div>

    <template #footer>
      <div class="flex items-center justify-between gap-3">
        <span class="text-xs text-faint">{{ t('prefs.persist') }}</span>
        <button
          type="button"
          class="inline-flex h-10 items-center rounded-full bg-surface-alt px-4 text-sm font-semibold tap hover:bg-line"
          @click="emit('update:open', false)"
        >
          {{ t('prefs.done') }}
        </button>
      </div>
    </template>
  </AppSheet>
</template>
