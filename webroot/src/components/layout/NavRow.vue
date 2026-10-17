<script setup lang="ts">
import { useRouter } from 'vue-router';

import AppIcon from '../ui/AppIcon.vue';
import AppBadge from '../ui/AppBadge.vue';

const props = defineProps<{
  icon: string;
  title: string;
  description?: string;
  to: string;
  badge?: string;
  badgeTone?: 'neutral' | 'ok' | 'warn' | 'error' | 'accent';
  disabled?: boolean;
}>();

const router = useRouter();

function open() {
  if (props.disabled) return;
  void router.push(props.to);
}
</script>

<template>
  <button
    type="button"
    class="flex w-full items-center gap-3 px-4 py-3 text-left tap disabled:cursor-default disabled:opacity-50"
    :class="'hover:bg-sunken active:bg-surface-alt'"
    :disabled="props.disabled"
    @click="open"
  >
    <span class="grid h-10 w-10 shrink-0 place-items-center rounded-tile bg-accent-soft text-accent">
      <AppIcon :name="props.icon" :size="20" />
    </span>
    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
      <span class="flex items-center gap-2">
        <span class="truncate text-[15px] font-semibold leading-5">{{ props.title }}</span>
        <AppBadge v-if="props.badge" :tone="props.badgeTone ?? 'neutral'">{{ props.badge }}</AppBadge>
      </span>
      <span v-if="props.description" class="line-clamp-2 text-xs leading-relaxed text-muted">
        {{ props.description }}
      </span>
    </span>
    <AppIcon name="chevron" :size="16" class="text-faint" />
  </button>
</template>
