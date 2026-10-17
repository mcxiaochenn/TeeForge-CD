<script setup lang="ts">
import { computed, onMounted } from 'vue';

import DeviceHeader from '../components/layout/DeviceHeader.vue';
import NavRow from '../components/layout/NavRow.vue';
import SectionCard from '../components/layout/SectionCard.vue';
import AppIcon from '../components/ui/AppIcon.vue';
import { formatVersion } from '../lib/format';
import { t } from '../lib/i18n';
import { hasHost } from '../lib/ksu';
import { loadSession, session } from '../state/session';

const hostMissing = !hasHost();

onMounted(() => {
  void loadSession();
});

const sourceLabel = computed(() => {
  if (!session.data) return '—';
  return session.data.config.keybox_source === 'rkp' ? t('source.rkp') : t('source.remote');
});
const modeLabel = computed(() => {
  if (!session.data) return '—';
  return session.data.config.target_mode === 'manual' ? t('mode.manual') : t('mode.auto');
});
</script>

<template>
  <div class="flex flex-col gap-6 pt-1">
    <div v-if="session.error" class="flex items-start gap-2 rounded-card bg-warning-soft px-4 py-3 text-xs leading-relaxed text-warning">
      <AppIcon name="alert" :size="16" class="mt-0.5" />
      <span>{{ t('home.bridgeMissing') }}</span>
    </div>

    <DeviceHeader :data="session.data" :loading="session.loading" />

    <SectionCard :title="t('home.section.manage')" plain>
      <div class="divide-y divide-line overflow-hidden rounded-card border border-line bg-surface shadow-card">
        <NavRow
          icon="apps"
          :title="t('entry.target.title')"
          :description="t('entry.target.desc')"
          :badge="modeLabel"
          :disabled="hostMissing"
          to="/target"
        />
        <NavRow
          icon="key"
          :title="t('entry.keybox.title')"
          :description="t('entry.keybox.desc')"
          :badge="sourceLabel"
          badge-tone="accent"
          :disabled="hostMissing"
          to="/keybox"
        />
        <NavRow
          icon="lock"
          :title="t('entry.rkp.title')"
          :description="t('entry.rkp.desc')"
          :badge="session.data?.capabilities.rkp_artifacts ? t('common.ready') : undefined"
          :badge-tone="session.data?.capabilities.rkp_artifacts ? 'ok' : 'neutral'"
          :disabled="hostMissing"
          to="/rkp"
        />
        <NavRow
          icon="download"
          :title="t('entry.update.title')"
          :description="t('entry.update.desc')"
          :badge="session.data ? formatVersion(session.data.module.version) : undefined"
          :disabled="hostMissing"
          to="/update"
        />
      </div>
    </SectionCard>

    <SectionCard :title="t('home.section.status')">
      <dl class="divide-y divide-line">
        <div class="flex items-center justify-between gap-3 py-2.5 first:pt-0 last:pb-0">
          <dt class="text-[13px] text-muted">{{ t('status.keyboxSource') }}</dt>
          <dd class="text-[13px] font-semibold">{{ sourceLabel }}</dd>
        </div>
        <div class="flex items-center justify-between gap-3 py-2.5">
          <dt class="text-[13px] text-muted">{{ t('status.targetMode') }}</dt>
          <dd class="text-[13px] font-semibold">{{ modeLabel }}</dd>
        </div>
        <div class="flex items-center justify-between gap-3 py-2.5">
          <dt class="text-[13px] text-muted">{{ t('status.rkpArtifacts') }}</dt>
          <dd class="text-[13px] font-semibold">
            {{ session.data?.capabilities.rkp_artifacts ? t('common.ready') : t('common.absent') }}
          </dd>
        </div>
        <div class="flex items-center justify-between gap-3 py-2.5 last:pb-0">
          <dt class="text-[13px] text-muted">{{ t('status.module') }}</dt>
          <dd class="text-right text-[13px] font-semibold">
            <template v-if="session.data">
              {{ formatVersion(session.data.module.version) }} ({{ session.data.module.version_code }})
            </template>
            <template v-else>—</template>
          </dd>
        </div>
      </dl>
    </SectionCard>
  </div>
</template>
