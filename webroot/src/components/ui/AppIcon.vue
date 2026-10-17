<script setup lang="ts">
/** 线性图标集：统一 24 视框、1.8 描边，全部为项目自绘路径。 */
const props = withDefaults(defineProps<{ name: string; size?: number }>(), { size: 20 });

type IconDef = { paths?: string[]; circles?: [number, number, number][]; rects?: [number, number, number, number, number][] };

const icons: Record<string, IconDef> = {
  back: { paths: ['M15 5l-7 7 7 7'] },
  chevron: { paths: ['M9 5l7 7-7 7'] },
  sliders: {
    paths: ['M4 7.5h9', 'M18 7.5h2', 'M4 16.5h2', 'M11 16.5h9'],
    circles: [
      [15.5, 7.5, 2.2],
      [8.5, 16.5, 2.2],
    ],
  },
  key: {
    paths: ['M10.4 14 20 4.4', 'M16.4 8 19 10.6', 'M13.8 10.6 16 12.8'],
    circles: [[7.5, 16.5, 3.5]],
  },
  apps: {
    rects: [
      [4, 4, 7, 7, 2],
      [13, 4, 7, 7, 2],
      [4, 13, 7, 7, 2],
      [13, 13, 7, 7, 2],
    ],
  },
  shield: { paths: ['M12 3.2 19 6.2v5c0 4-3 7.6-7 9.1-4-1.5-7-5.1-7-9.1v-5z'] },
  refresh: { paths: ['M20 11a8 8 0 1 0-2.4 5.7', 'M20 5v6h-6'] },
  trash: { paths: ['M4 7h16', 'M9 7V5h6v2', 'M6.5 7 7.5 20h9L17.5 7', 'M10 11v5', 'M14 11v5'] },
  copy: {
    paths: ['M15 6.6A2.6 2.6 0 0 0 12.4 4H6.6A2.6 2.6 0 0 0 4 6.6v5.8A2.6 2.6 0 0 0 6.6 15'],
    rects: [[9, 9, 11, 11, 2.6]],
  },
  check: { paths: ['M4.8 12.6 9.6 17.4 19.2 6.6'] },
  close: { paths: ['M6 6l12 12M18 6 6 18'] },
  alert: {
    paths: ['M10.3 4.4 2.9 17.2A2 2 0 0 0 4.6 20.2h14.8a2 2 0 0 0 1.7-3L13.7 4.4a2 2 0 0 0-3.4 0Z', 'M12 9.4v4.2', 'M12 16.7h.01'],
  },
  search: { paths: ['M16.6 16.6 21 21'], circles: [[11, 11, 6.6]] },
  download: { paths: ['M12 4v11', 'M7.5 10.5 12 15l4.5-4.5', 'M5 19h14'] },
  info: { paths: ['M12 11v5.5', 'M12 7.8h.01'], circles: [[12, 12, 8.6]] },
  lock: { paths: ['M8 10V7.6a4 4 0 0 1 8 0V10'], rects: [[4.5, 10, 15, 10, 2.4]] },
  file: { paths: ['M6.5 3h6.6L18.5 8.4V21h-12z', 'M13 3v5.6h5.5'] },
};
</script>

<template>
  <svg
    :width="props.size"
    :height="props.size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
    class="shrink-0"
  >
    <path v-for="(d, index) in icons[props.name]?.paths ?? []" :key="`p${index}`" :d="d" />
    <circle
      v-for="(circle, index) in icons[props.name]?.circles ?? []"
      :key="`c${index}`"
      :cx="circle[0]"
      :cy="circle[1]"
      :r="circle[2]"
    />
    <rect
      v-for="(rect, index) in icons[props.name]?.rects ?? []"
      :key="`r${index}`"
      :x="rect[0]"
      :y="rect[1]"
      :width="rect[2]"
      :height="rect[3]"
      :rx="rect[4]"
    />
  </svg>
</template>
