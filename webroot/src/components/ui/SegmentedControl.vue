<script setup lang="ts">
interface Option {
  value: string;
  label: string;
  disabled?: boolean;
}

const props = defineProps<{ modelValue: string; options: Option[]; label?: string }>();
const emit = defineEmits<{ (event: 'update:modelValue', value: string): void }>();
</script>

<template>
  <div
    class="flex gap-0.5 rounded-full p-[3px]"
    :style="{ background: 'var(--seg-track)' }"
    role="group"
    :aria-label="props.label"
  >
    <button
      v-for="option in props.options"
      :key="option.value"
      type="button"
      class="h-9 min-w-0 flex-1 truncate rounded-full px-1.5 text-[13px] font-semibold tap"
      :class="
        props.modelValue === option.value
          ? 'text-fg shadow-card'
          : 'text-muted hover:text-fg'
      "
      :style="props.modelValue === option.value ? { background: 'var(--seg-thumb)' } : undefined"
      :aria-pressed="props.modelValue === option.value"
      :disabled="option.disabled"
      @click="emit('update:modelValue', option.value)"
    >
      {{ option.label }}
    </button>
  </div>
</template>
