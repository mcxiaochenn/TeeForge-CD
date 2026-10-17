<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';

import AppButton from '../components/ui/AppButton.vue';
import AppIcon from '../components/ui/AppIcon.vue';
import AppSheet from '../components/ui/AppSheet.vue';
import SectionCard from '../components/layout/SectionCard.vue';
import { api } from '../lib/api';
import { t } from '../lib/i18n';
import { loadSession, session } from '../state/session';

const router = useRouter();
const source = ref<'rkp' | 'remote'>('remote');
const artifacts = ref(false);
const busy = ref(false);
const error = ref('');
const log = ref('');
const result = ref('');
const confirmOpen = ref(false);

const rkpAvailable = computed(() => artifacts.value);
const canApply = computed(() => source.value === 'remote' || rkpAvailable.value);

async function refresh() {
  try {
    const info = await api.rkpArtifacts();
    artifacts.value = info.exists;
  } catch {
    artifacts.value = false;
  }
}

onMounted(async () => {
  await loadSession();
  source.value = session.data?.config.keybox_source ?? 'remote';
  await refresh();
});

function choose(next: 'rkp' | 'remote') {
  if (next === 'rkp' && !rkpAvailable.value) return;
  source.value = next;
}

async function apply() {
  busy.value = true;
  error.value = '';
  result.value = '';
  log.value = '';
  try {
    const report = await api.keybox(source.value, (chunk) => {
      log.value += chunk;
    });
    result.value = t('keybox.result', { n: report.targets.length });
    confirmOpen.value = false;
    await loadSession(true);
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="flex flex-col gap-5 pt-1">
    <p class="px-1 text-xs leading-relaxed text-muted">{{ t('keybox.intro') }}</p>

    <div class="flex flex-col gap-2.5">
      <div
        role="button"
        tabindex="0"
        class="rounded-card border p-4 text-left tap"
        :class="[
          source === 'rkp'
            ? 'border-accent bg-accent-soft'
            : 'border-line bg-surface hover:border-line-strong',
          rkpAvailable ? 'cursor-pointer' : 'cursor-default opacity-70',
        ]"
        :aria-pressed="source === 'rkp'"
        :aria-disabled="!rkpAvailable"
        @click="choose('rkp')"
        @keydown.enter.prevent="choose('rkp')"
        @keydown.space.prevent="choose('rkp')"
      >
        <div class="flex items-start gap-3">
          <span
            class="mt-1 grid h-4 w-4 shrink-0 place-items-center rounded-full border-2"
            :class="source === 'rkp' ? 'border-accent' : 'border-line-strong'"
            aria-hidden="true"
          >
            <span v-if="source === 'rkp'" class="h-2 w-2 rounded-full bg-accent" />
          </span>
          <div class="min-w-0 flex-1">
            <p class="flex items-center gap-2 text-[15px] font-semibold">
              {{ t('keybox.rkp.title') }}
              <span v-if="!rkpAvailable" class="rounded-full bg-surface-alt px-2 py-0.5 text-[10px] text-muted">
                {{ t('keybox.rkp.missing') }}
              </span>
            </p>
            <p class="mt-1 text-xs leading-relaxed text-muted">{{ t('keybox.rkp.desc') }}</p>
            <span
              v-if="!rkpAvailable"
              role="link"
              tabindex="0"
              class="mt-2 inline-block text-xs font-semibold text-accent underline underline-offset-2"
              @click.stop="router.push('/rkp')"
              @keydown.enter.stop.prevent="router.push('/rkp')"
            >
              {{ t('keybox.rkp.goIssue') }}
            </span>
          </div>
        </div>
      </div>

      <div
        role="button"
        tabindex="0"
        class="cursor-pointer rounded-card border p-4 text-left tap"
        :class="
          source === 'remote'
            ? 'border-accent bg-accent-soft'
            : 'border-line bg-surface hover:border-line-strong'
        "
        :aria-pressed="source === 'remote'"
        @click="choose('remote')"
        @keydown.enter.prevent="choose('remote')"
        @keydown.space.prevent="choose('remote')"
      >
        <div class="flex items-start gap-3">
          <span
            class="mt-1 grid h-4 w-4 shrink-0 place-items-center rounded-full border-2"
            :class="source === 'remote' ? 'border-accent' : 'border-line-strong'"
            aria-hidden="true"
          >
            <span v-if="source === 'remote'" class="h-2 w-2 rounded-full bg-accent" />
          </span>
          <div class="min-w-0 flex-1">
            <p class="text-[15px] font-semibold">{{ t('keybox.remote.title') }}</p>
            <p class="mt-1 text-xs leading-relaxed text-muted">{{ t('keybox.remote.desc') }}</p>
          </div>
        </div>
      </div>
    </div>

    <p v-if="error" class="rounded-card bg-danger-soft px-4 py-3 text-xs leading-relaxed text-danger">
      {{ error }}
    </p>
    <p v-if="result" class="rounded-card bg-accent-soft px-4 py-3 text-xs font-medium text-accent">
      {{ result }}
    </p>

    <pre
      v-if="log"
      class="max-h-52 overflow-y-auto rounded-card border border-line bg-sunken p-3 font-mono text-[11px] leading-relaxed text-muted"
      >{{ log }}</pre
    >

    <AppButton block size="lg" :disabled="!canApply || busy" :loading="busy" @click="confirmOpen = true">
      {{ t('keybox.apply') }}
    </AppButton>

    <SectionCard :title="t('keybox.notes')">
      <ul class="flex flex-col gap-2 text-xs leading-relaxed text-muted">
        <li class="flex gap-2"><AppIcon name="info" :size="14" class="mt-0.5 text-faint" />{{ t('keybox.note.overwrite') }}</li>
        <li class="flex gap-2"><AppIcon name="info" :size="14" class="mt-0.5 text-faint" />{{ t('keybox.note.backends') }}</li>
      </ul>
    </SectionCard>

    <AppSheet v-model:open="confirmOpen" :title="t('keybox.confirm.title')">
      <p class="text-sm leading-relaxed text-muted">
        {{ source === 'rkp' ? t('keybox.confirm.rkp') : t('keybox.confirm.remote') }}
      </p>
      <template #footer>
        <div class="flex justify-end gap-2">
          <AppButton variant="ghost" @click="confirmOpen = false">{{ t('common.cancel') }}</AppButton>
          <AppButton :loading="busy" @click="apply">{{ t('common.confirm') }}</AppButton>
        </div>
      </template>
    </AppSheet>
  </div>
</template>
