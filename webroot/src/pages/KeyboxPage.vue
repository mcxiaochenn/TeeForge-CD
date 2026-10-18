<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';

import AppButton from '../components/ui/AppButton.vue';
import AppIcon from '../components/ui/AppIcon.vue';
import AppSheet from '../components/ui/AppSheet.vue';
import SegmentedControl from '../components/ui/SegmentedControl.vue';
import SectionCard from '../components/layout/SectionCard.vue';
import { api } from '../lib/api';
import { t } from '../lib/i18n';
import { loadSession, session } from '../state/session';

const router = useRouter();
const source = ref<'rkp' | 'remote'>('remote');
const remoteSlot = ref<'1' | '2' | '3'>('1');
const force = ref(false);
const slotOptions = computed(() => ['1', '2', '3'].map(value => ({ value, label: t('keybox.slot.option', { n: value }), disabled: busy.value })));
function selectSlot(value: string) {
  if (!busy.value && (value === '1' || value === '2' || value === '3')) remoteSlot.value = value;
}
function confirm(rewrite: boolean) {
  if (busy.value) return;
  force.value = rewrite;
  confirmOpen.value = true;
}
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
  remoteSlot.value = String(session.data?.config.keybox_remote_slot ?? 1) as '1' | '2' | '3';
  await refresh();
});

function choose(next: 'rkp' | 'remote') {
  if (busy.value) return;
  if (next === 'rkp' && !rkpAvailable.value) return;
  source.value = next;
}

async function apply() {
  if (busy.value) return;
  busy.value = true;
  error.value = '';
  result.value = '';
  log.value = '';
  try {
    const report = await api.keybox(source.value, Number(remoteSlot.value) as 1 | 2 | 3, force.value, (chunk) => {
      log.value += chunk;
    });
    result.value = t('keybox.result', { n: report.targets.filter(target => target.changed).length });
    confirmOpen.value = false;
    await loadSession(true);
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
    confirmOpen.value = false;
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
        :tabindex="busy ? -1 : 0"
        class="rounded-card border p-4 text-left tap"
        :class="[
          source === 'rkp'
            ? 'border-accent bg-accent-soft'
            : 'border-line bg-surface hover:border-line-strong',
          rkpAvailable ? 'cursor-pointer' : 'cursor-default opacity-70',
        ]"
        :aria-pressed="source === 'rkp'"
        :aria-disabled="busy || !rkpAvailable"
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
              :tabindex="busy ? -1 : 0"
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
        :tabindex="busy ? -1 : 0"
        class="cursor-pointer rounded-card border p-4 text-left tap"
        :class="
          source === 'remote'
            ? 'border-accent bg-accent-soft'
            : 'border-line bg-surface hover:border-line-strong'
        "
        :aria-pressed="source === 'remote'"
        :aria-disabled="busy"
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

    <SectionCard v-if="source === 'remote'" :title="t('keybox.slot.title')">
      <SegmentedControl :model-value="remoteSlot" :options="slotOptions" :label="t('keybox.slot.title')" @update:model-value="selectSlot" />
      <p class="mt-2 text-xs leading-relaxed text-muted">{{ t('keybox.slot.note') }}</p>
    </SectionCard>

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

    <AppButton block size="lg" :disabled="!canApply || busy" :loading="busy" @click="confirm(false)">
      {{ t('keybox.apply') }}
    </AppButton>

    <AppButton v-if="source === 'remote'" block size="lg" variant="tonal" :disabled="busy" @click="confirm(true)">
      {{ t('keybox.force') }}
    </AppButton>

    <SectionCard :title="t('keybox.notes')">
      <ul class="flex flex-col gap-2 text-xs leading-relaxed text-muted">
        <li class="flex gap-2"><AppIcon name="info" :size="14" class="mt-0.5 text-faint" />{{ t('keybox.note.overwrite') }}</li>
        <li class="flex gap-2"><AppIcon name="info" :size="14" class="mt-0.5 text-faint" />{{ t('keybox.note.backends') }}</li>
      </ul>
    </SectionCard>

    <AppSheet :open="confirmOpen" :title="force ? t('keybox.force') : t('keybox.confirm.title')" @update:open="value => { if (!busy) confirmOpen = value; }">
      <p class="text-sm leading-relaxed text-muted">
        {{ source === 'rkp' ? t('keybox.confirm.rkp') : t('keybox.confirm.remote', { n: remoteSlot }) }}
      </p>
      <p v-if="force" class="mt-3 text-sm leading-relaxed text-muted">{{ t('keybox.confirm.force') }}</p>
      <template #footer>
        <div class="flex justify-end gap-2">
          <AppButton variant="ghost" :disabled="busy" @click="confirmOpen = false">{{ t('common.cancel') }}</AppButton>
          <AppButton :loading="busy" @click="apply">{{ t('common.confirm') }}</AppButton>
        </div>
      </template>
    </AppSheet>
  </div>
</template>
