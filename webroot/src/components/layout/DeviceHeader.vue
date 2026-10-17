<script setup lang="ts">
import { computed } from 'vue';

import type { Bootstrap } from '../../lib/api';
import { t } from '../../lib/i18n';
import AppBadge from '../ui/AppBadge.vue';
import AppIcon from '../ui/AppIcon.vue';

const props = defineProps<{ data: Bootstrap | null; loading: boolean }>();

const chips = computed(() => {
  const data = props.data;
  if (!data) return [];
  const items = [
    data.root.method ? `${data.root.method}` : '',
    data.device.soc_model || data.device.soc_manufacturer || '',
    data.device.abi || '',
    data.device.security_patch
      ? `${t('device.patch')} ${data.device.security_patch}`
      : '',
    data.system.selinux ? `SELinux · ${data.system.selinux}` : '',
    data.device.verified_boot ? `VB · ${data.device.verified_boot}` : '',
  ];
  return items.filter(Boolean);
});

const androidLine = computed(() => {
  const data = props.data;
  if (!data) return '';
  if (!data.device.android && !data.device.sdk) return '';
  return t('device.android', {
    release: data.device.android || '?',
    sdk: data.device.sdk || '?',
  });
});
</script>

<template>
  <section class="rounded-card border border-line bg-surface p-4 shadow-card">
    <div v-if="props.loading && !props.data" class="flex items-center gap-3">
      <span class="h-12 w-12 animate-pulse rounded-full bg-surface-alt" />
      <div class="flex-1 space-y-2">
        <span class="block h-5 w-2/3 animate-pulse rounded bg-surface-alt" />
        <span class="block h-3 w-1/2 animate-pulse rounded bg-surface-alt" />
      </div>
    </div>

    <template v-else-if="props.data">
      <div class="flex items-center gap-3">
        <span class="grid h-12 w-12 shrink-0 place-items-center rounded-full bg-surface-alt text-accent">
          <AppIcon name="shield" :size="24" />
        </span>
        <div class="min-w-0 flex-1">
          <p class="truncate text-[22px] font-semibold leading-7">
            {{ props.data.device.name || t('device.unknown') }}
          </p>
          <p class="truncate text-[13px] text-muted">{{ androidLine }}</p>
        </div>
        <AppBadge tone="neutral">v{{ props.data.module.version }}</AppBadge>
      </div>

      <div class="mt-3 flex flex-wrap gap-1.5">
        <span
          v-for="chip in chips"
          :key="chip"
          class="inline-flex h-7 max-w-full items-center gap-1 truncate rounded-full border border-line px-2.5 text-[11px] text-muted"
        >
          {{ chip }}
        </span>
      </div>
    </template>

    <p v-else class="py-4 text-center text-sm text-muted">{{ t('common.unavailable') }}</p>
  </section>
</template>
