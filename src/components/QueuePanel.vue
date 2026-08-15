<script setup lang="ts">
import { store, fmtTime, playQueue, setPlayMode } from "../stores/player";
import { play } from "../player";
import PIcon from "./PIcon.vue";
import CoverImg from "./CoverImg.vue";

function playAt(i: number) {
  const song = store.queue[i];
  if (!song) return;
  playQueue(store.queue.slice(), i);
  play(song.path).catch(() => {});
}

function removeAt(i: number) {
  const song = store.queue[i];
  if (!song) return;
  if (i <= store.currentIndex) store.currentIndex--;
  store.queue.splice(i, 1);
  if (store.currentIndex >= store.queue.length) store.currentIndex = store.queue.length - 1;
  if (store.playMode === "shuffle") setPlayMode("shuffle");
}

function clearQueue() {
  if (store.current) {
    store.queue = [store.current];
    store.currentIndex = 0;
  } else {
    store.queue = [];
    store.currentIndex = -1;
  }
}
</script>

<template>
  <Transition name="qmask">
    <div v-if="store.queueOpen" class="q-mask" @click="store.queueOpen = false">
      <div class="q-panel card" @click.stop>
        <div class="q-head">
          <span class="q-title">播放队列</span>
          <span class="q-count">{{ store.queue.length }} 首</span>
          <button class="btn small" :disabled="store.queue.length <= 1" @click="clearQueue">清空</button>
          <button class="q-close" title="关闭" @click="store.queueOpen = false">✕</button>
        </div>
        <div id="queue-list" class="q-list">
          <div
            v-for="(s, i) in store.queue"
            :key="s.path + i"
            class="q-row"
            :class="{ cur: i === store.currentIndex }"
            @click="playAt(i)"
          >
            <span class="q-idx">
              <PIcon v-if="i === store.currentIndex" name="play" tone="amber" :size="11" />
              <template v-else>{{ i + 1 }}</template>
            </span>
            <CoverImg :path="s.path" :size="30" lazy />
            <div class="q-meta">
              <div class="q-name">{{ s.title }}</div>
              <div class="q-artist">{{ s.artist || s.album }}</div>
            </div>
            <span class="q-dur">{{ fmtTime(s.duration) }}</span>
            <button class="q-x" title="从队列移除" @click.stop="removeAt(i)">✕</button>
          </div>
          <div v-if="store.queue.length === 0" class="q-empty">队列为空</div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.q-mask {
  position: fixed;
  inset: 0;
  z-index: 80;
  background: rgba(0, 0, 0, 0.25);
}
.q-panel {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: min(320px, 90vw);
  display: flex;
  flex-direction: column;
  background: linear-gradient(180deg, #2a2d36 0%, #22252d 100%);
  border-left: 1px solid var(--line);
  border-radius: 0;
  box-shadow: -12px 0 36px rgba(0, 0, 0, 0.45);
}
.q-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px 10px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  flex-shrink: 0;
}
.q-title {
  font-size: 14px;
  font-weight: 700;
  color: #f5e3bc;
  flex: 1;
}
.q-count {
  font-size: 11px;
  color: var(--text-dim);
}
.q-close {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.5);
  background: linear-gradient(180deg, #4a4f5a 0%, #34373f 100%);
  color: var(--text-dim);
  font-size: 11px;
  line-height: 1;
  cursor: pointer;
}
.q-close:hover {
  filter: brightness(1.2);
}
.q-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px 0;
}
.q-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
  cursor: pointer;
  transition: background 0.12s;
}
.q-row:hover {
  background: rgba(255, 255, 255, 0.05);
}
.q-row.cur {
  background: rgba(232, 163, 61, 0.12);
}
.q-idx {
  width: 16px;
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-dim);
  text-align: center;
  display: inline-flex;
  justify-content: center;
}
.q-meta {
  flex: 1;
  min-width: 0;
}
.q-name {
  font-size: 12.5px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.q-row.cur .q-name {
  color: #f2d9a8;
  font-weight: 600;
}
.q-artist {
  font-size: 10.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.q-dur {
  font-size: 10.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}
.q-x {
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  border: none;
  background: none;
  color: var(--text-dim);
  font-size: 11px;
  cursor: pointer;
  border-radius: 50%;
}
.q-x:hover {
  color: #ff9b8a;
  background: rgba(224, 108, 90, 0.18);
}
.q-empty {
  padding: 40px 0;
  text-align: center;
  font-size: 12px;
  color: var(--text-dim);
}
.qmask-enter-active,
.qmask-leave-active {
  transition: opacity 0.2s ease;
}
.qmask-enter-active .q-panel,
.qmask-leave-active .q-panel {
  transition: transform 0.25s ease;
}
.qmask-enter-from,
.qmask-leave-to {
  opacity: 0;
}
.qmask-enter-from .q-panel,
.qmask-leave-to .q-panel {
  transform: translateX(100%);
}
</style>
