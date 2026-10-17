<script setup lang="ts">
import { computed } from 'vue';
import { useRoute } from 'vue-router';

import AppBar from './components/layout/AppBar.vue';
import { t } from './lib/i18n';

const route = useRoute();
const title = computed(() => t(String(route.meta.titleKey ?? 'app.title')));
const isHome = computed(() => route.name === 'home');
</script>

<template>
  <div class="shell safe-x min-h-dvh">
    <AppBar :title="title" :show-back="!isHome" />
    <main class="mx-auto w-full max-w-[560px] px-4 pb-10">
      <router-view />
    </main>
  </div>
</template>
