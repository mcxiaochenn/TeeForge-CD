<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';

import SectionCard from '../components/layout/SectionCard.vue';
import AppBadge from '../components/ui/AppBadge.vue';
import AppButton from '../components/ui/AppButton.vue';
import AppIcon from '../components/ui/AppIcon.vue';
import AppSheet from '../components/ui/AppSheet.vue';
import SegmentedControl from '../components/ui/SegmentedControl.vue';
import { ApiError, api, type UpdateInfo, type UpdateInstall } from '../lib/api';
import { copyText } from '../lib/clipboard';
import { formatBytes, formatVersion } from '../lib/format';
import { t } from '../lib/i18n';
import { loadSession, session } from '../state/session';

const info = ref<UpdateInfo | null>(null);
const channel = ref<'stable' | 'dev'>('stable');
const loading = ref(false);
const busy = ref(false);
const error = ref('');
const log = ref('');
const installResult = ref<UpdateInstall | null>(null);
const confirmOpen = ref(false);
const copied = ref('');

const available = computed(() => info.value?.available ?? false);
const latestSize = computed(() => formatBytes(info.value?.latest.size ?? 0));

async function check() {
  loading.value = true;
  error.value = '';
  try {
    info.value = await api.updateCheck();
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  } finally {
    loading.value = false;
  }
}

async function switchChannel(next: string) {
  if (next === channel.value || loading.value || busy.value) return;
  channel.value = next as 'stable' | 'dev';
  error.value = '';
  try {
    await api.updateChannel(channel.value);
    await check();
  } catch (failure) {
    error.value = failure instanceof Error ? failure.message : String(failure);
  }
}

async function install() {
  busy.value = true;
  error.value = '';
  log.value = '';
  installResult.value = null;
  confirmOpen.value = false;
  try {
    installResult.value = await api.updateInstall((chunk) => {
      log.value += chunk;
    });
    await loadSession(true);
    await check();
  } catch (failure) {
    if (failure instanceof ApiError) error.value = failure.message;
    else error.value = failure instanceof Error ? failure.message : String(failure);
  } finally {
    busy.value = false;
  }
}

async function copy(text: string, tag: string) {
  const ok = await copyText(text);
  copied.value = ok ? tag : '';
  window.setTimeout(() => {
    if (copied.value === tag) copied.value = '';
  }, 1600);
}

onMounted(async () => {
  await loadSession();
  const configured = session.data?.config.update_channel;
  channel.value = configured === 'dev' ? 'dev' : 'stable';
  await check();
});
</script>

<template>
  <div class="flex flex-col gap-5 pt-1">
    <SectionCard :title="t('update.section.module')">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-[15px] font-semibold">
            {{ t('update.current') }}
            <span class="font-mono">{{ formatVersion(info?.current?.version ?? session.data?.module.version) }}</span>
          </p>
          <p class="mt-0.5 text-xs text-muted">
            versionCode {{ info?.current.version_code ?? session.data?.module.version_code ?? '—' }}
          </p>
        </div>
        <AppBadge :tone="available ? 'warn' : 'ok'">
          {{ available ? t('update.available') : t('update.upToDate') }}
        </AppBadge>
      </div>

      <div class="mt-4 flex flex-col gap-2">
        <span class="text-[11px] font-bold uppercase tracking-[0.09em] text-muted">
          {{ t('update.channel') }}
        </span>
        <SegmentedControl
          :model-value="channel"
          :label="t('update.channel')"
          :options="[
            { value: 'stable', label: t('update.channel.stable') },
            { value: 'dev', label: t('update.channel.dev') },
          ]"
          @update:model-value="switchChannel"
        />
      </div>

      <AppButton variant="tonal" block class="mt-4" :loading="loading" @click="check">
        <AppIcon name="refresh" :size="16" />
        {{ t('update.check') }}
      </AppButton>
    </SectionCard>

    <p v-if="error" class="rounded-card bg-danger-soft px-4 py-3 text-xs leading-relaxed text-danger">
      {{ error }}
    </p>

    <SectionCard v-if="available && info" :title="t('update.section.latest')">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <p class="text-[15px] font-semibold">{{ formatVersion(info.latest.version) }}</p>
          <p class="mt-0.5 text-xs text-muted">
            versionCode {{ info.latest.version_code }} · {{ latestSize }}
          </p>
        </div>
        <AppBadge :tone="info.latest.sha256 ? 'ok' : 'warn'">
          {{ info.latest.sha256 ? t('update.signed') : t('update.unverified') }}
        </AppBadge>
      </div>

      <div class="mt-3 flex flex-col gap-2">
        <div class="flex items-center justify-between gap-2 rounded-tile bg-sunken px-3 py-2">
          <span class="min-w-0 truncate font-mono text-[10px] text-faint">{{ info.endpoint }}</span>
          <button
            type="button"
            class="shrink-0 text-[11px] font-semibold text-accent"
            @click="copy(info.endpoint, 'endpoint')"
          >
            {{ copied === 'endpoint' ? t('common.copied') : t('common.copy') }}
          </button>
        </div>

        <div v-if="info.latest.changelog_url" class="flex items-center justify-between gap-2 rounded-tile bg-sunken px-3 py-2">
          <span class="min-w-0 truncate font-mono text-[10px] text-faint">{{ info.latest.changelog_url }}</span>
          <button
            type="button"
            class="shrink-0 text-[11px] font-semibold text-accent"
            @click="copy(info.latest.changelog_url, 'changelog')"
          >
            {{ copied === 'changelog' ? t('common.copied') : t('common.copy') }}
          </button>
        </div>

        <p v-if="!info.latest.sha256" class="rounded-tile bg-warning-soft px-3 py-2 text-[11px] leading-relaxed text-warning">
          {{ t('update.noChecksum') }}
        </p>
      </div>

      <AppButton block size="lg" class="mt-4" :loading="busy" @click="confirmOpen = true">
        <AppIcon name="download" :size="18" />
        {{ t('update.install') }}
      </AppButton>
    </SectionCard>

    <SectionCard v-else-if="!loading && info" :title="t('update.section.latest')">
      <p class="py-2 text-center text-xs text-muted">{{ t('update.none') }}</p>
    </SectionCard>

    <pre
      v-if="log"
      class="max-h-52 overflow-y-auto rounded-card border border-line bg-sunken p-3 font-mono text-[11px] leading-relaxed text-muted"
      >{{ log }}</pre
    >

    <section
      v-if="installResult"
      class="rounded-card px-4 py-3 text-xs leading-relaxed"
      :class="installResult.installed ? 'bg-accent-soft text-accent' : 'bg-warning-soft text-warning'"
    >
      <p class="font-semibold">
        {{ installResult.installed ? t('update.installed') : t('update.notInstalled') }}
      </p>
      <p v-if="installResult.installed" class="mt-1">
        {{ t('update.reboot') }}
        <span v-if="installResult.installer" class="opacity-70">· {{ installResult.installer }}</span>
      </p>
      <p v-else-if="installResult.reason === 'checksum_missing'" class="mt-1">
        {{ t('update.savedManual') }}
      </p>
      <p v-if="installResult.saved_to" class="mt-1 break-all font-mono text-[10px]">
        {{ installResult.saved_to }}
      </p>
    </section>

    <AppSheet
      v-model:open="confirmOpen"
      :title="t('update.confirm.title')"
      :subtitle="formatVersion(info?.latest?.version)"
    >
      <p class="text-sm leading-relaxed text-muted">
        {{ info?.latest.sha256 ? t('update.confirm.body') : t('update.confirm.unverified') }}
      </p>
      <template #footer>
        <div class="flex justify-end gap-2">
          <AppButton variant="ghost" @click="confirmOpen = false">{{ t('common.cancel') }}</AppButton>
          <AppButton :loading="busy" @click="install">{{ t('common.confirm') }}</AppButton>
        </div>
      </template>
    </AppSheet>
  </div>
</template>
