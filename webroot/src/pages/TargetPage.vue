<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';

import SegmentedControl from '../components/ui/SegmentedControl.vue';
import AppButton from '../components/ui/AppButton.vue';
import AppIcon from '../components/ui/AppIcon.vue';
import AppSheet from '../components/ui/AppSheet.vue';
import EmptyState from '../components/ui/EmptyState.vue';
import { api, type TargetList } from '../lib/api';
import { t } from '../lib/i18n';
import { appLabels, iconUrl } from '../lib/ksu';
import { loadSession, session } from '../state/session';

const list = ref<TargetList | null>(null);
const labels = ref<Record<string, string>>({});
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const query = ref('');
const selected = ref<Set<string>>(new Set());
const confirmOpen = ref(false);
const mode = ref<'auto' | 'manual'>('auto');

const filtered = computed(() => {
  const items = list.value?.packages ?? [];
  const keyword = query.value.trim().toLowerCase();
  if (!keyword) return items;
  return items.filter(
    (item) =>
      item.name.toLowerCase().includes(keyword) ||
      (labels.value[item.name] ?? '').toLowerCase().includes(keyword),
  );
});

const selectedCount = computed(() => selected.value.size);
const allSelected = computed(
  () => filtered.value.length > 0 && filtered.value.every((item) => selected.value.has(item.name)),
);

const backendNames = computed(() => {
  const map: Record<string, string> = {
    tricky: 'Tricky Store',
    teesim: 'TEESimulator',
    omk: 'Oh My Keymint',
  };
  return (list.value?.backends ?? []).map((name) => map[name] ?? name);
});

function hideIcon(event: Event) {
  const image = event.target as HTMLImageElement | null;
  if (image) image.style.display = 'none';
}

function toggle(name: string) {
  const next = new Set(selected.value);
  if (next.has(name)) next.delete(name);
  else next.add(name);
  selected.value = next;
}

function toggleAll() {
  const next = new Set(selected.value);
  if (allSelected.value) {
    for (const item of filtered.value) next.delete(item.name);
  } else {
    for (const item of filtered.value) next.add(item.name);
  }
  selected.value = next;
}

async function load() {
  loading.value = true;
  error.value = '';
  try {
    const data = await api.targetList();
    list.value = data;
    mode.value = data.mode;
    selected.value = new Set(data.packages.filter((item) => item.selected).map((item) => item.name));
    const names = data.packages.map((item) => item.name);
    labels.value = await appLabels(names);
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  } finally {
    loading.value = false;
  }
}

async function switchMode(next: string) {
  if (next === mode.value || busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    if (next === 'manual') {
      // 进入手动模式以"当前全量"为基线，避免空集合直接清空目标列表。
      const names = (list.value?.packages ?? []).map((item) => item.name);
      await api.targetApply(names);
      mode.value = 'manual';
      selected.value = new Set(names);
    } else {
      await api.targetMode('auto');
      mode.value = 'auto';
      selected.value = new Set((list.value?.packages ?? []).map((item) => item.name));
    }
    await loadSession(true);
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  } finally {
    busy.value = false;
  }
}

async function apply() {
  busy.value = true;
  error.value = '';
  try {
    await api.targetApply([...selected.value]);
    confirmOpen.value = false;
    mode.value = 'manual';
    await loadSession(true);
    await load();
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  } finally {
    busy.value = false;
  }
}

onMounted(load);
</script>

<template>
  <div class="flex flex-col gap-4 pt-1">
    <div class="flex flex-col gap-2">
      <SegmentedControl
        :model-value="mode"
        :label="t('target.mode.label')"
        :options="[
          { value: 'auto', label: t('target.mode.auto') },
          { value: 'manual', label: t('target.mode.manual') },
        ]"
        @update:model-value="switchMode"
      />
      <p class="px-1 text-[11px] leading-relaxed text-faint">
        {{ mode === 'auto' ? t('target.mode.autoHint') : t('target.mode.manualHint') }}
      </p>
    </div>

    <div class="flex items-center gap-2">
      <label class="relative flex h-10 flex-1 items-center gap-2 rounded-full border border-line bg-surface px-3">
        <AppIcon name="search" :size="16" class="text-faint" />
        <input
          v-model="query"
          type="search"
          :placeholder="t('target.search')"
          class="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-faint"
        />
      </label>
      <AppButton variant="tonal" size="sm" @click="toggleAll">
        {{ allSelected ? t('common.clear') : t('common.selectAll') }}
      </AppButton>
    </div>

    <div v-if="backendNames.length" class="flex flex-wrap gap-1.5 px-1">
      <span
        v-for="name in backendNames"
        :key="name"
        class="rounded-full bg-accent-soft px-2 py-0.5 text-[11px] font-medium text-accent"
      >
        {{ name }}
      </span>
    </div>

    <p v-if="error" class="rounded-card bg-danger-soft px-4 py-3 text-xs leading-relaxed text-danger">
      {{ error }}
    </p>

    <div
      v-if="loading"
      class="flex flex-col items-center gap-2 rounded-card border border-line bg-surface py-10 text-xs text-muted"
    >
      <span class="h-5 w-5 animate-spin rounded-full border-2 border-accent border-t-transparent" />
      {{ t('target.loading') }}
    </div>

    <EmptyState
      v-else-if="!filtered.length"
      icon="apps"
      :title="query ? t('target.noMatch') : t('target.empty')"
      :hint="query ? '' : t('target.emptyHint')"
    />

    <ul v-else class="divide-y divide-line overflow-hidden rounded-card border border-line bg-surface">
      <li v-for="item in filtered" :key="item.name">
        <label class="flex cursor-pointer items-center gap-3 px-3 py-2.5 tap hover:bg-sunken">
          <input
            type="checkbox"
            class="peer sr-only"
            :checked="selected.has(item.name)"
            @change="toggle(item.name)"
          />
          <span
            class="grid h-5 w-5 shrink-0 place-items-center rounded-md border-2 border-line-strong text-transparent peer-checked:border-accent peer-checked:bg-accent peer-checked:text-on-accent"
            aria-hidden="true"
          >
            <AppIcon name="check" :size="13" />
          </span>
          <span class="relative grid h-9 w-9 shrink-0 place-items-center overflow-hidden rounded-[10px] bg-surface-alt text-xs font-semibold text-muted">
            {{ item.name.slice(0, 1).toUpperCase() }}
            <img
              :src="iconUrl(item.name)"
              alt=""
              class="absolute inset-0 h-full w-full object-cover"
              loading="lazy"
              @error="hideIcon"
            />
          </span>
          <span class="min-w-0 flex-1">
            <span class="block truncate text-[14px] font-medium leading-5">
              {{ labels[item.name] || item.name }}
            </span>
            <span
              v-if="labels[item.name] && labels[item.name] !== item.name"
              class="block truncate font-mono text-[11px] text-faint"
            >
              {{ item.name }}
            </span>
          </span>
        </label>
      </li>
    </ul>

    <div
      class="sticky bottom-0 z-10 -mx-4 mt-1 flex items-center justify-between gap-3 border-t border-line px-4 pb-2 pt-3 backdrop-blur-xl"
      :style="{ background: 'var(--bar-bg)' }"
    >
      <span class="text-xs text-muted">
        {{ t('target.selected', { n: selectedCount }) }}
        <span class="text-faint">/ {{ list?.counts.installed ?? 0 }}</span>
      </span>
      <AppButton size="md" :disabled="mode !== 'manual' || busy" :loading="busy" @click="confirmOpen = true">
        {{ t('common.apply') }}
      </AppButton>
    </div>

    <AppSheet
      v-model:open="confirmOpen"
      :title="t('target.confirm.title')"
      :subtitle="t('target.selected', { n: selectedCount })"
    >
      <p class="text-sm leading-relaxed text-muted">{{ t('target.confirm.body') }}</p>
      <ul v-if="backendNames.length" class="mt-3 flex flex-col gap-1.5">
        <li v-for="name in backendNames" :key="name" class="flex items-center gap-2 text-xs text-muted">
          <AppIcon name="check" :size="14" class="text-accent" />
          {{ name }}
        </li>
      </ul>
      <template #footer>
        <div class="flex justify-end gap-2">
          <AppButton variant="ghost" @click="confirmOpen = false">{{ t('common.cancel') }}</AppButton>
          <AppButton :loading="busy" @click="apply">{{ t('common.confirm') }}</AppButton>
        </div>
      </template>
    </AppSheet>
  </div>
</template>
