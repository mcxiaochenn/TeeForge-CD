<script setup lang="ts">
const props = withDefaults(
  defineProps<{ tone?: 'neutral' | 'ok' | 'warn' | 'error' | 'accent' }>(),
  { tone: 'neutral' },
);

const tones = {
  neutral: 'bg-surface-alt text-muted',
  ok: 'bg-accent-soft text-accent',
  accent: 'bg-accent-soft text-accent',
  warn: 'bg-warning-soft text-warning',
  error: 'bg-danger-soft text-danger',
};
</script>

<template>
  <span
    class="inline-flex h-6 shrink-0 items-center gap-1 rounded-full px-2.5 text-[11px] font-semibold tracking-wide"
    :class="tones[props.tone]"
  >
    <slot />
  </span>
</template>
