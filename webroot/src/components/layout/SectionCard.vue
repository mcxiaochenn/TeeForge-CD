<script setup lang="ts">
withDefaults(defineProps<{ title?: string; description?: string; plain?: boolean }>(), {
  title: '',
  description: '',
  plain: false,
});
</script>

<template>
  <section class="flex flex-col gap-2">
    <header v-if="title || description || $slots.action" class="flex min-h-6 items-center gap-3 px-1">
      <div class="min-w-0 flex-1">
        <h2 v-if="title" class="text-[11px] font-bold uppercase tracking-[0.09em] text-muted">
          {{ title }}
        </h2>
        <p v-if="description" class="mt-0.5 text-xs leading-relaxed text-faint">{{ description }}</p>
      </div>
      <slot name="action" />
    </header>

    <div v-if="plain" class="flex flex-col gap-2"><slot /></div>
    <div v-else class="rounded-card border border-line bg-surface p-4 shadow-card"><slot /></div>
  </section>
</template>
