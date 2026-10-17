<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'tonal' | 'ghost' | 'danger';
    size?: 'sm' | 'md' | 'lg';
    block?: boolean;
    disabled?: boolean;
    loading?: boolean;
  }>(),
  { variant: 'primary', size: 'md', block: false, disabled: false, loading: false },
);

const base =
  'inline-flex items-center justify-center gap-2 rounded-full font-semibold whitespace-nowrap tap active:scale-[0.97] disabled:cursor-default disabled:opacity-45 disabled:active:scale-100';

const variants = {
  primary: 'bg-accent text-on-accent hover:bg-[var(--accent-press)]',
  tonal: 'bg-surface-alt text-fg hover:bg-line',
  ghost: 'text-muted hover:bg-surface-alt hover:text-fg',
  danger: 'bg-danger-soft text-danger hover:bg-[var(--danger-soft)]',
};

const sizes = {
  sm: 'h-9 px-3.5 text-[13px]',
  md: 'h-10 px-4 text-sm',
  lg: 'h-11 px-5 text-[15px]',
};

const classes = computed(() => [
  base,
  variants[props.variant],
  sizes[props.size],
  props.block ? 'w-full' : '',
]);
</script>

<template>
  <button type="button" :class="classes" :disabled="props.disabled || props.loading">
    <span
      v-if="props.loading"
      class="h-4 w-4 shrink-0 animate-spin rounded-full border-2 border-current border-t-transparent"
      aria-hidden="true"
    />
    <slot />
  </button>
</template>
