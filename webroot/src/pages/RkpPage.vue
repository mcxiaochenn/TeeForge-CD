<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';

import SectionCard from '../components/layout/SectionCard.vue';
import AppBadge from '../components/ui/AppBadge.vue';
import AppButton from '../components/ui/AppButton.vue';
import AppIcon from '../components/ui/AppIcon.vue';
import AppSheet from '../components/ui/AppSheet.vue';
import { ApiError, api, type RkpArtifacts, type RkpStatus } from '../lib/api';
import { formatBytes, formatUnix, shortFingerprint } from '../lib/format';
import { hasKey, t } from '../lib/i18n';
import { loadSession } from '../state/session';

const status = ref<RkpStatus | null>(null);
const artifacts = ref<RkpArtifacts | null>(null);
const loading = ref(true);
const busy = ref(false);
const log = ref('');
const failure = ref<ApiError | null>(null);
const issueError = ref<ApiError | null>(null);
const confirmOpen = ref(false);
const deleteOpen = ref(false);
const succeeded = ref('');

const supported = computed(() => status.value?.supported ?? false);

const rows = computed(() => {
  const data = status.value;
  if (!data) return [];
  const yes = t('common.yes');
  const no = t('common.no');
  return [
    { label: t('rkp.row.platform'), value: `${data.platform.os}/${data.platform.arch}` },
    { label: t('rkp.row.root'), value: `${data.root.method} ${data.root.version}`.trim() },
    {
      label: t('rkp.row.soc'),
      value: [data.soc.manufacturer, data.soc.model].filter(Boolean).join(' ') || t('common.unknown'),
    },
    { label: t('rkp.row.socState'), value: data.soc.state },
    { label: t('rkp.row.library'), value: data.library.present ? data.library.path : no, mono: true },
    { label: t('rkp.row.smcinvoke'), value: data.device.present ? data.device.path : no, mono: true },
    { label: t('rkp.row.fingerprint'), value: data.fingerprint || t('common.unknown'), mono: true },
  ];
});

const meta = computed(() => artifacts.value?.meta ?? null);
const createdAt = computed(() => formatUnix(Number(meta.value?.created_at ?? 0)));
const certificates = computed(() => Number(meta.value?.certificates ?? 0));
const deviceId = computed(() => String(meta.value?.device_id ?? ''));

const explain = computed(() => {
  // hint 是后端给出的解释入口，优先于错误码；帧类错误没有数字码，只能走 hint。
  const hint = issueError.value?.hint;
  if (hint && hasKey(hint)) return t(hint);
  const code = issueError.value?.code;
  if (!code) return '';
  const key = `rkp.code.${code}`;
  return hasKey(key) ? t(key) : '';
});

function blockerText(id: string, detail: string): string {
  const key = `rkp.blocker.${id}`;
  return hasKey(key) ? t(key) : detail;
}

function toApiError(error: unknown): ApiError {
  if (error instanceof ApiError) return error;
  return new ApiError({ message: error instanceof Error ? error.message : String(error) });
}

async function load() {
  loading.value = true;
  failure.value = null;
  try {
    status.value = await api.rkpStatus();
    artifacts.value = await api.rkpArtifacts();
  } catch (error) {
    failure.value = toApiError(error);
  } finally {
    loading.value = false;
  }
}

async function issue() {
  busy.value = true;
  log.value = '';
  issueError.value = null;
  succeeded.value = '';
  confirmOpen.value = false;
  try {
    await api.rkpIssue((chunk) => {
      log.value += chunk;
    });
    succeeded.value = t('rkp.issue.done');
    await load();
    await loadSession(true);
  } catch (error) {
    issueError.value = toApiError(error);
  } finally {
    busy.value = false;
  }
}

async function removeArtifacts() {
  busy.value = true;
  try {
    await api.rkpDelete();
    deleteOpen.value = false;
    succeeded.value = '';
    await load();
    await loadSession(true);
  } catch (error) {
    failure.value = toApiError(error);
  } finally {
    busy.value = false;
  }
}

onMounted(load);
</script>

<template>
  <div class="flex flex-col gap-5 pt-1">
    <div class="flex items-start gap-2 rounded-card bg-warning-soft px-4 py-3 text-[11px] leading-relaxed text-warning">
      <AppIcon name="alert" :size="15" class="mt-0.5" />
      <span>{{ t('rkp.warning') }}</span>
    </div>

    <div v-if="loading" class="flex items-center justify-center gap-2 rounded-card border border-line bg-surface py-10 text-xs text-muted">
      <span class="h-5 w-5 animate-spin rounded-full border-2 border-accent border-t-transparent" />
      {{ t('common.loading') }}
    </div>

    <template v-else-if="status">
      <SectionCard :title="t('rkp.section.status')">
        <div class="flex items-center justify-between gap-3">
          <AppBadge :tone="supported ? 'ok' : 'error'">
            {{ supported ? t('rkp.met') : t('rkp.unmet') }}
          </AppBadge>
          <button type="button" class="text-xs font-medium text-accent" @click="load">
            {{ t('common.refresh') }}
          </button>
        </div>

        <ul v-if="status.blockers.length" class="mt-3 flex flex-col gap-1.5 rounded-tile bg-danger-soft p-3">
          <li v-for="blocker in status.blockers" :key="blocker.id" class="flex gap-2 text-[11px] leading-relaxed text-danger">
            <span class="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-danger" />
            {{ blockerText(blocker.id, blocker.detail) }}
          </li>
        </ul>

        <dl class="mt-3 divide-y divide-line">
          <div
            v-for="row in rows"
            :key="row.label"
            class="py-2 first:pt-0"
            :class="row.mono ? 'flex flex-col gap-1' : 'flex items-baseline justify-between gap-3'"
          >
            <dt class="shrink-0 text-[12px] text-muted">{{ row.label }}</dt>
            <dd
              class="min-w-0 break-all text-[12px] font-medium"
              :class="row.mono ? 'font-mono text-[11px] text-fg' : 'text-right'"
            >
              {{ row.value }}
            </dd>
          </div>
        </dl>

        <details v-if="status.soc.evidence.length" class="mt-3 border-t border-line pt-3">
          <summary class="cursor-pointer text-xs font-semibold text-muted">{{ t('rkp.evidence') }}</summary>
          <ul class="mt-2 flex flex-col gap-1">
            <li v-for="item in status.soc.evidence" :key="item" class="break-all font-mono text-[11px] text-faint">
              {{ item }}
            </li>
          </ul>
        </details>
      </SectionCard>

      <AppButton block size="lg" :disabled="!supported || busy" :loading="busy" @click="confirmOpen = true">
        {{ t('rkp.issue.button') }}
      </AppButton>

      <pre
        v-if="log"
        class="max-h-56 overflow-y-auto rounded-card border border-line bg-sunken p-3 font-mono text-[11px] leading-relaxed text-muted"
        >{{ log }}</pre
      >

      <p v-if="succeeded" class="rounded-card bg-accent-soft px-4 py-3 text-xs font-medium text-accent">
        {{ succeeded }}
      </p>

      <section v-if="issueError" class="rounded-card border border-[var(--danger-soft)] bg-surface p-4">
        <div class="flex items-center gap-2">
          <AppIcon name="alert" :size="16" class="text-danger" />
          <p class="text-sm font-semibold text-danger">{{ t('rkp.error.title') }}</p>
        </div>
        <p class="mt-2 text-[11px] font-mono text-faint">
          {{ t('rkp.error.stage') }}: {{ issueError.stage || 'unknown' }}
          <template v-if="issueError.code"> · {{ t('rkp.error.code') }}: {{ issueError.code }}</template>
        </p>
        <p class="mt-2 text-[12px] leading-relaxed text-fg">{{ issueError.message }}</p>
        <p v-if="explain" class="mt-2 rounded-tile bg-warning-soft px-3 py-2 text-[11px] leading-relaxed text-warning">
          {{ explain }}
        </p>
        <details v-if="issueError.raw" class="mt-3">
          <summary class="cursor-pointer text-[11px] font-semibold text-muted">{{ t('rkp.error.raw') }}</summary>
          <pre class="mt-2 overflow-x-auto rounded-tile bg-sunken p-2 font-mono text-[10px] leading-relaxed text-faint">{{ JSON.stringify(issueError.raw, null, 2) }}</pre>
        </details>
      </section>

      <p v-if="failure" class="rounded-card bg-danger-soft px-4 py-3 text-xs leading-relaxed text-danger">
        {{ failure.message }}
      </p>

      <SectionCard :title="t('rkp.section.artifacts')">
        <template v-if="artifacts?.exists">
          <dl class="divide-y divide-line">
            <div class="flex items-center justify-between gap-3 pb-2">
              <dt class="text-[12px] text-muted">{{ t('rkp.artifact.created') }}</dt>
              <dd class="text-[12px] font-medium">{{ createdAt }}</dd>
            </div>
            <div class="flex items-center justify-between gap-3 py-2">
              <dt class="text-[12px] text-muted">{{ t('rkp.artifact.certificates') }}</dt>
              <dd class="text-[12px] font-medium">{{ certificates }}</dd>
            </div>
            <div v-if="meta?.frame_layout" class="flex items-center justify-between gap-3 py-2">
              <dt class="text-[12px] text-muted">{{ t('rkp.frame.layout') }}</dt>
              <dd class="font-mono text-[11px]">{{ meta.frame_layout }}</dd>
            </div>
            <div class="flex items-center justify-between gap-3 py-2">
              <dt class="text-[12px] text-muted">{{ t('rkp.artifact.deviceId') }}</dt>
              <dd class="font-mono text-[11px]">{{ shortFingerprint(deviceId, 8) }}</dd>
            </div>
            <div class="flex items-center justify-between gap-3 py-2">
              <dt class="text-[12px] text-muted">{{ t('rkp.artifact.size') }}</dt>
              <dd class="text-[12px] font-medium">{{ formatBytes(artifacts.keybox?.size) }}</dd>
            </div>
            <div class="flex items-start justify-between gap-3 pt-2">
              <dt class="shrink-0 text-[12px] text-muted">{{ t('rkp.artifact.dir') }}</dt>
              <dd class="break-all text-right font-mono text-[11px] text-faint">{{ artifacts.dir }}</dd>
            </div>
          </dl>
          <AppButton variant="danger" block class="mt-4" :disabled="busy" @click="deleteOpen = true">
            <AppIcon name="trash" :size="16" />
            {{ t('rkp.delete.button') }}
          </AppButton>
        </template>
        <p v-else class="py-2 text-center text-xs text-muted">{{ t('rkp.artifact.empty') }}</p>
      </SectionCard>
    </template>

    <AppSheet v-model:open="confirmOpen" :title="t('rkp.confirm.title')">
      <p class="text-sm leading-relaxed text-muted">{{ t('rkp.confirm.body') }}</p>
      <template #footer>
        <div class="flex justify-end gap-2">
          <AppButton variant="ghost" @click="confirmOpen = false">{{ t('common.cancel') }}</AppButton>
          <AppButton :loading="busy" @click="issue">{{ t('common.confirm') }}</AppButton>
        </div>
      </template>
    </AppSheet>

    <AppSheet v-model:open="deleteOpen" :title="t('rkp.delete.title')">
      <p class="text-sm leading-relaxed text-muted">{{ t('rkp.delete.body') }}</p>
      <template #footer>
        <div class="flex justify-end gap-2">
          <AppButton variant="ghost" @click="deleteOpen = false">{{ t('common.cancel') }}</AppButton>
          <AppButton variant="danger" :loading="busy" @click="removeArtifacts">{{ t('common.delete') }}</AppButton>
        </div>
      </template>
    </AppSheet>
  </div>
</template>
