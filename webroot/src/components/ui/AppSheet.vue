<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';

import AppIcon from './AppIcon.vue';

const props = defineProps<{ open: boolean; title?: string; subtitle?: string }>();
const emit = defineEmits<{ (event: 'update:open', value: boolean): void }>();

const panel = ref<HTMLElement | null>(null);

function close() {
  emit('update:open', false);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && props.open) close();
}

watch(
  () => props.open,
  (open) => {
    document.body.style.overflow = open ? 'hidden' : '';
    if (open) panel.value?.focus();
  },
);

onMounted(() => window.addEventListener('keydown', onKeydown));
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown);
  document.body.style.overflow = '';
});
</script>

<template>
  <Teleport to="body">
    <transition name="sheet">
      <div
        v-if="props.open"
        class="fixed inset-0 z-50 flex items-end justify-center bg-black/45 sm:items-center"
        @click.self="close"
      >
        <div
          ref="panel"
          tabindex="-1"
          role="dialog"
          aria-modal="true"
          class="sheet-panel flex w-full max-w-[560px] flex-col overflow-hidden rounded-t-sheet bg-surface shadow-sheet outline-none sm:rounded-sheet"
        >
          <div class="mx-auto mt-2.5 h-1 w-9 rounded-full bg-line-strong sm:hidden" aria-hidden="true" />
          <header class="flex items-start gap-3 px-5 pb-2 pt-3">
            <div class="min-w-0 flex-1">
              <h2 class="text-[17px] font-bold leading-6">{{ props.title }}</h2>
              <p v-if="props.subtitle" class="mt-0.5 truncate font-mono text-xs text-faint">
                {{ props.subtitle }}
              </p>
            </div>
            <button
              type="button"
              class="-mr-1 grid h-9 w-9 place-items-center rounded-full text-muted tap hover:bg-surface-alt hover:text-fg"
              aria-label="close"
              @click="close"
            >
              <AppIcon name="close" :size="18" />
            </button>
          </header>

          <div class="min-h-0 flex-1 overflow-y-auto px-5 pb-4">
            <slot />
          </div>

          <footer v-if="$slots.footer" class="border-t border-line px-5 pt-3 safe-bottom">
            <slot name="footer" />
          </footer>
        </div>
      </div>
    </transition>
  </Teleport>
</template>
