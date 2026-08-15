<script setup lang="ts">
type IconName =
  | "prev" | "next" | "play" | "pause" | "vol" | "search" | "note" | "mini" | "songs" | "album" | "artist"
  | "loop" | "shuffle" | "repeat-one" | "heart" | "heart-o" | "list" | "plus" | "folder" | "info";
const props = withDefaults(
  defineProps<{ name: IconName; tone?: "dark" | "light" | "amber"; size?: number }>(),
  { tone: "light" },
);
// 固定 id：同名同色渐变只渲染一份 defs，避免列表场景产生海量重复节点
const gid = `picon-${props.name}-${props.tone}`;
const defaults: Record<string, number> = {
  prev: 15, next: 15, play: 18, pause: 18, vol: 15, search: 14, note: 17, mini: 13, songs: 15, album: 15, artist: 15,
  loop: 18, shuffle: 18, "repeat-one": 18, heart: 16, "heart-o": 16, list: 15, plus: 15, folder: 15, info: 16,
};
const size = props.size ?? defaults[props.name] ?? 16;
const stops = {
  dark: ["#3a2504", "#241a04"],
  light: ["#ffffff", "#9aa0ab"],
  amber: ["#e8a33d", "#b37f2b"],
}[props.tone];
</script>

<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" class="picon" aria-hidden="true">
    <defs>
      <linearGradient :id="gid" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" :stop-color="stops[0]" />
        <stop offset="1" :stop-color="stops[1]" />
      </linearGradient>
    </defs>
    <g :fill="`url(#${gid})`">
      <path v-if="name === 'prev'" d="M6 4h2.2v16H6zM20 4.6v14.8L8.8 12z" />
      <path v-if="name === 'next'" d="M15.8 4h2.2v16h-2.2zM4 4.6v14.8L15.2 12z" />
      <path v-if="name === 'play'" d="M8 4.5v15l12-7.5z" />
      <path v-if="name === 'pause'" d="M7.5 4.5h3.4v15H7.5zM13.1 4.5h3.4v15h-3.4z" />
      <path v-if="name === 'vol'" d="M3.5 10v4.4h4l4.9 3.9V6.1l-4.9 3.9z" />
      <path v-if="name === 'search'" d="M10 3.5a6.5 6.5 0 1 0 3.9 11.7l4.9 4.9 1.8-1.8-4.9-4.9A6.5 6.5 0 0 0 10 3.5zm0 2.2a4.3 4.3 0 1 1 0 8.6 4.3 4.3 0 0 1 0-8.6z" />
      <path v-if="name === 'note'" d="M9 3.5v10.6A3.4 3.4 0 0 0 8 14a3.2 3.2 0 1 0 3.2 3.2V7.2l7-1.6v5.4a3.4 3.4 0 0 0-1-.1 3.2 3.2 0 1 0 3.2 3.2V2.3z" />
      <path v-if="name === 'mini'" d="M4 4h11v3H7v8H4zM11 10h9v10h-9z" />
      <path v-if="name === 'songs'" d="M4 5h16v2.2H4zM4 10.9h16v2.2H4zM4 16.8h11v2.2H4z" />
      <path v-if="name === 'album'" d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19zm0 3.2a6.3 6.3 0 1 1 0 12.6 6.3 6.3 0 0 1 0-12.6zm0 3.4a2.9 2.9 0 1 0 0 5.8 2.9 2.9 0 0 0 0-5.8z" />
      <path v-if="name === 'artist'" d="M12 12.5a4.3 4.3 0 1 0 0-8.6 4.3 4.3 0 0 0 0 8.6zM4.5 21c0-4.1 3.4-7.5 7.5-7.5s7.5 3.4 7.5 7.5v.5H4.5z" />
      <path v-if="name === 'loop'" d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z" />
      <path v-if="name === 'shuffle'" d="M14.5 4l2 2L5 17.5 6.5 19 18 7.5l2 2V4h-5.5zM6.5 5L5 6.5l4.5 4.5L11 9.5 6.5 5zm6.5 9l-1 1 4.5 4.5 1.5-1.5L13 14z" />
      <path v-if="name === 'repeat-one'" d="M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4zM13.7 8.9l-2.8-1.4v1.4zM10.9 8.9h2.8v7h-2.8zM10.3 15.9h5.4v1.2h-5.4z" />
      <path v-if="name === 'heart'" d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
      <path v-if="name === 'list'" d="M4 5.5h2.2v2.2H4zM8.6 5.5H20v2.2H8.6zM4 10.9h2.2v2.2H4zM8.6 10.9H20v2.2H8.6zM4 16.3h2.2v2.2H4zM8.6 16.3H20v2.2H8.6z" />
      <path v-if="name === 'plus'" d="M11 4.2h2v6.8H19.8v2H13V19.8h-2V13H4.2v-2H11z" />
      <path v-if="name === 'folder'" d="M2.5 5.2c0-.7.6-1.3 1.3-1.3h5.6l2.2 2.3h8.6c.7 0 1.3.6 1.3 1.3v11c0 .7-.6 1.3-1.3 1.3H3.8c-.7 0-1.3-.6-1.3-1.3z" />
      <path v-if="name === 'info'" d="M12 2.5a9.5 9.5 0 1 0 0 19 9.5 9.5 0 0 0 0-19zm0 3.4a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3zm-1.8 5h3.6v7h-3.6z" />
    </g>
    <g v-if="name === 'heart-o'" fill="none" :stroke="`url(#${gid})`" stroke-width="1.9" stroke-linejoin="round">
      <path d="M12 20.4l-1.3-1.2C5.6 14.8 2.6 11.9 2.6 8.6 2.6 5.8 4.9 3.5 7.7 3.5c1.5 0 2.9.7 3.9 1.8h.8c1-.1 2.4-.8 3.9-.8 2.8 0 5.1 2.3 5.1 5.1 0 3.3-3 6.2-8.1 10.6L12 20.4z" />
    </g>
  </svg>
</template>

<style scoped>
.picon {
  display: block;
  filter: drop-shadow(0 1px 1px rgba(0, 0, 0, 0.45));
}
</style>