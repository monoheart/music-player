<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { store, fmtTime, playQueue, setPlayMode } from "../stores/player";
import { play } from "../player";
import { toggleFav } from "../playback";
import type { Song } from "../types";
import PIcon from "./PIcon.vue";
import CoverImg from "./CoverImg.vue";

const props = defineProps<{ songs: Song[]; reorderable?: boolean; dragOrder?: boolean }>();
const emit = defineEmits<{
  (e: "contextmenu", ev: MouseEvent, song: Song, index: number): void;
  (e: "reorder", songs: Song[]): void;
}>();
const tbodyRef = ref<HTMLElement | null>(null);

const hasPlaying = computed(() => props.songs.some((s) => s.path === store.current?.path));

const btnVisible = ref(false);
let contentEl: HTMLElement | null = null;

function updateBtnVisible() {
  const row = tbodyRef.value?.querySelector<HTMLElement>("tr.playing");
  const container = row?.closest<HTMLElement>(".content");
  if (!row || !container) {
    btnVisible.value = false;
    return;
  }
  const cRect = container.getBoundingClientRect();
  const rRect = row.getBoundingClientRect();
  const inView = rRect.top >= cRect.top && rRect.bottom <= cRect.bottom;
  btnVisible.value = !inView;
}

onMounted(() => {
  contentEl = tbodyRef.value?.closest<HTMLElement>(".content") ?? null;
  contentEl?.addEventListener("scroll", updateBtnVisible, { passive: true });
  updateBtnVisible();
});

onBeforeUnmount(() => {
  contentEl?.removeEventListener("scroll", updateBtnVisible);
});

watch(
  [() => props.songs, () => store.current?.path],
  () => nextTick(updateBtnVisible),
);

function jumpToPlaying() {
  const row = tbodyRef.value?.querySelector<HTMLElement>("tr.playing");
  const container = row?.closest<HTMLElement>(".content");
  if (!row || !container) return;
  const cRect = container.getBoundingClientRect();
  const rRect = row.getBoundingClientRect();
  const target = container.scrollTop + (rRect.top - cRect.top) - container.clientHeight / 2 + row.clientHeight / 2;
  container.scrollTo({ top: Math.max(0, target), behavior: "smooth" });
}

function dbl(song: Song) {
  const i = props.songs.indexOf(song);
  if (i < 0) return;
  playQueue(props.songs.slice(), i);
  play(song.path).catch(() => {});
}

function rowCtx(e: MouseEvent, song: Song, i: number) {
  emit("contextmenu", e, song, i);
}

function toggleFavLocal(song: Song, e: Event) {
  e.stopPropagation();
  toggleFav(song).catch(() => {});
}

// ---- 长按左键 + 上下拖动换序（歌单详情未搜索时 / 全部歌曲自定义排序时） ----
function canDrag(): boolean {
  return props.reorderable || props.dragOrder;
}
const dragRow = ref(-1);
let dragState: {
  row: number;
  moving: boolean;
  startX: number;
  startY: number;
} | null = null;
let suppressClick = false;
const MOVE_TOL = 4;

function getScrollParent(el: HTMLElement | null): HTMLElement | null {
  while (el) {
    if (el.scrollHeight > el.clientHeight) return el;
    el = el.parentElement;
  }
  return null;
}

function rowPointerDown(e: PointerEvent, i: number) {
  if (!canDrag() || e.button !== 0) return;
  dragState = { row: i, moving: false, startX: e.clientX, startY: e.clientY };
  window.addEventListener("pointermove", dragMove);
  window.addEventListener("pointerup", dragUp);
  window.addEventListener("pointercancel", dragCancel);
}

function dragMove(e: PointerEvent) {
  if (!dragState) return;
  if (!dragState.moving) {
    if (Math.abs(e.clientX - dragState.startX) <= MOVE_TOL && Math.abs(e.clientY - dragState.startY) <= MOVE_TOL) {
      return;
    }
    dragState.moving = true;
    dragRow.value = dragState.row;
    suppressClick = true;
    setTimeout(() => {
      suppressClick = false;
    }, 500);
    document.body.style.userSelect = "none";
  }
  const tbody = tbodyRef.value;
  const n = props.songs.length;
  if (!tbody || n === 0) return;
  const rect = tbody.getBoundingClientRect();
  const first = tbody.firstElementChild as HTMLElement | null;
  const rowH = first ? first.getBoundingClientRect().height : 50;
  let target = Math.floor((e.clientY - rect.top) / rowH);
  if (target < 0) target = 0;
  if (target > n - 1) target = n - 1;
  pendingTarget = target;
  if (!rafId) rafId = requestAnimationFrame(applyMove);
  const sp = getScrollParent(tbody);
  if (sp) {
    const r = sp.getBoundingClientRect();
    if (e.clientY < r.top + 36) sp.scrollTop -= 14;
    else if (e.clientY > r.bottom - 36) sp.scrollTop += 14;
  }
}

// 移动换序合并到每帧执行一次，避免鼠标高频移动触发大量全表重排
let pendingTarget = -1;
let rafId = 0;
function applyMove() {
  rafId = 0;
  if (!dragState || pendingTarget < 0) return;
  const target = pendingTarget;
  pendingTarget = -1;
  if (target === dragState.row) return;
  const arr = props.songs;
  const [m] = arr.splice(dragState.row, 1);
  arr.splice(target, 0, m);
  dragState.row = target;
  dragRow.value = target;
}

function dragUp() {
  if (!dragState) return;
  const wasMoving = dragState.moving;
  const arr = props.songs;
  dragCancel();
  if (!wasMoving) return;
  emit("reorder", arr);
  // 队列若与主列表同源，同步重排
  const queueFull = store.queue.length === arr.length &&
    store.queue.every((s, i) => s.path === arr[i].path);
  if (queueFull) store.queue = arr;
  if (store.playMode === "shuffle") setPlayMode("shuffle");
}

function dragCancel() {
  if (!dragState) return;
  if (rafId) {
    cancelAnimationFrame(rafId);
    rafId = 0;
  }
  pendingTarget = -1;
  window.removeEventListener("pointermove", dragMove);
  window.removeEventListener("pointerup", dragUp);
  window.removeEventListener("pointercancel", dragCancel);
  dragState = null;
  dragRow.value = -1;
  document.body.style.userSelect = "";
}

function rowClick() {
  if (suppressClick) {
    suppressClick = false;
  }
}
</script>

<template>
  <div class="table-wrap">
    <button v-if="btnVisible" class="jump-btn" title="回到当前歌曲" @click="jumpToPlaying">
      <PIcon name="play" tone="amber" :size="11" />
      <span>回到当前歌曲</span>
    </button>
    <table class="song-table">
      <thead>
        <tr>
          <th class="col-title">标题</th>
          <th class="col-artist">艺术家</th>
          <th class="col-album">专辑</th>
          <th class="col-dur">时长</th>
          <th class="col-fav"></th>
        </tr>
      </thead>
    <tbody ref="tbodyRef">
      <tr
        v-for="(song, i) in songs"
        :key="song.path"
        :class="{ playing: song.path === store.current?.path, dragging: dragRow === i, 'can-drag': canDrag() }"
        @dblclick="dbl(song)"
        @click="rowClick"
        @contextmenu="rowCtx($event, song, i)"
        @pointerdown="rowPointerDown($event, i)"
      >
        <td class="col-title">
          <div class="title-cell">
            <span class="idx">{{ i + 1 }}</span>
            <CoverImg :path="song.path" :size="36" lazy />
            <span class="title-text">{{ song.title }}</span>
            <span v-if="song.path === store.current?.path" class="now-indicator">
              <PIcon name="play" tone="amber" :size="12" />
            </span>
          </div>
        </td>
        <td class="col-artist">{{ song.artist }}</td>
        <td class="col-album">{{ song.album }}</td>
        <td class="col-dur">{{ fmtTime(song.duration) }}</td>
        <td class="col-fav">
          <button
            class="fav-btn"
            :class="{ on: song.fav }"
            :title="song.fav ? '取消收藏' : '收藏'"
            @click="toggleFavLocal(song, $event)"
            @pointerdown.stop
          >
            <PIcon :name="song.fav ? 'heart' : 'heart-o'" :tone="song.fav ? 'amber' : 'light'" :size="15" />
          </button>
        </td>
      </tr>
    </tbody>
  </table>
  </div>
</template>

<style scoped>
.table-wrap {
  position: relative;
}
.jump-btn {
  position: sticky;
  top: 6px;
  right: 6px;
  float: right;
  z-index: 6;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 10px;
  border-radius: 12px;
  border: 1px solid rgba(0, 0, 0, 0.5);
  background: linear-gradient(180deg, #4a4f5a 0%, #34373f 100%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.15), 0 2px 6px rgba(0, 0, 0, 0.4);
  color: #f2d9a8;
  font-family: inherit;
  font-size: 11px;
  cursor: pointer;
}
.jump-btn:hover {
  filter: brightness(1.15);
}
.song-table {
  width: 100%;
  border-collapse: collapse;
  table-layout: fixed;
}
.song-table th {
  position: sticky;
  top: 0;
  z-index: 4;
  text-align: left;
  padding: 10px 12px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.5px;
  color: var(--text-dim);
  background: var(--card);
  border-bottom: 1px solid var(--line);
}
.song-table td {
  padding: 7px 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.song-table tbody tr {
  cursor: pointer;
}
.song-table tbody tr:hover td {
  background: rgba(255, 255, 255, 0.05);
}
.song-table tbody tr.playing td {
  background: rgba(232, 163, 61, 0.12);
}
.col-title {
  width: 34%;
}
.col-artist {
  width: 21%;
}
.col-album {
  width: 28%;
}
.col-dur {
  width: 8%;
  text-align: right;
  color: var(--text-dim);
}
.col-fav {
  width: 36px;
  text-align: center;
}
.fav-btn {
  border: none;
  background: none;
  cursor: pointer;
  padding: 4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0.45;
  transition: opacity 0.15s;
}
.fav-btn:hover {
  opacity: 1;
}
.fav-btn.on {
  opacity: 1;
}
tbody tr {
  transition: background 0.1s;
}
tbody tr.can-drag {
  cursor: grab;
}
tbody tr.dragging {
  opacity: 0.55;
}
.col-title .title-cell {
  display: flex;
  align-items: center;
  gap: 10px;
}
.col-title .idx {
  width: 14px;
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-dim);
  text-align: right;
}
.col-title .title-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.col-title .now-indicator {
  flex-shrink: 0;
}
.song-table tbody tr.playing .title-text {
  color: #f2d9a8;
  font-weight: 600;
}
</style>