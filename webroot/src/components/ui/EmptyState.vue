<script setup lang="ts">
import AppIcon from './AppIcon.vue';

withDefaults(defineProps<{ icon?: string; title: string; hint?: string }>(), { icon: 'info' });
</script>

<template>
  <div class="flex flex-col items-center gap-3 px-6 py-10 text-center">
    <span class="grid h-12 w-12 place-items-center rounded-full bg-surface-alt text-faint">
      <AppIcon :name="icon" :size="22" />
    </span>
    <p class="text-sm font-semibold">{{ title }}</p>
    <p v-if="hint" class="max-w-[26rem] text-xs leading-relaxed text-muted">{{ hint }}</p>
    <slot />
  </div>
</template>
