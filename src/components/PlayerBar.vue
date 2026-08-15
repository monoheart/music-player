<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { store, fmtTime } from "../stores/player";
import { togglePlay, nextTrack, prevTrack, setVolume } from "../player";
import { MODE_ICON, MODE_LABEL, cycleMode, useSeek, toggleFav, usePlaylistPicker } from "../playback";
import PIcon from "./PIcon.vue";
import CoverImg from "./CoverImg.vue";
import Marquee from "./Marquee.vue";

const progressEl = ref<HTMLDivElement | null>(null);
const { seekPct, seekDown } = useSeek(progressEl);

// 音量：按钮 + 点击弹出的竖向调节条，点别处自动收起
const volOpen = ref(false);
const volRailEl = ref<HTMLElement | null>(null);
let volDragging = false;
const volPct = computed(() => Math.round(store.volume * 100));
function volSetFromEvent(e: PointerEvent) {
  const t = volRailEl.value;
  if (!t) return;
  const rect = t.getBoundingClientRect();
  const ratio = 1 - Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
  setVolume(Math.round(ratio * 10) / 10);
}
function volRailDown(e: PointerEvent) {
  e.stopPropagation();
  volDragging = true;
  volSetFromEvent(e);
  window.addEventListener("pointermove", volRailMove);
  window.addEventListener("pointerup", volRailUp);
}
function volRailMove(e: PointerEvent) {
  if (volDragging) volSetFromEvent(e);
}
function volRailUp() {
  if (!volDragging) return;
  volDragging = false;
  window.removeEventListener("pointermove", volRailMove);
  window.removeEventListener("pointerup", volRailUp);
}

const { open: plOpen, playlists, menuRef, toggle: togglePl, add: addToPl, closeOutside } = usePlaylistPicker();
onMounted(() => {
  window.addEventListener("pointerdown", () => {
    volOpen.value = false;
  });
  window.addEventListener("pointerdown", closeOutside);
});
</script>

<template>
  <footer class="player-bar">
    <div class="pb-now" title="查看详情" @click="store.nowPlaying = true">
      <CoverImg v-if="store.current" :path="store.current.path" :size="56" />
      <span v-else class="pb-cover placeholder"><PIcon name="note" tone="amber" :size="22" /></span>
      <div class="pb-meta">
        <Marquee class="pb-title" :text="store.current?.title ?? '未在播放'" />
        <div class="pb-artist">{{ store.current?.artist ?? "—" }}</div>
      </div>
    </div>

    <div class="pb-fav">
      <button
        class="pb-fav-btn"
        :class="{ on: store.current?.fav }"
        :title="store.current?.fav ? '取消收藏' : '收藏'"
        @click="store.current && toggleFav(store.current)"
      >
        <PIcon
          :name="store.current?.fav ? 'heart' : 'heart-o'"
          :tone="store.current?.fav ? 'amber' : 'light'"
          :size="15"
        />
      </button>
      <button class="pb-fav-btn" title="加入歌单" @click="togglePl">
        <PIcon name="plus" :size="15" />
      </button>
      <div v-if="plOpen" ref="menuRef" class="pb-pl-menu card" @click.stop>
        <div class="pb-pl-title">加入歌单</div>
        <button v-for="p in playlists" :key="p.id" class="ctx-item" @click="addToPl(store.current, p.id)">
          <PIcon name="list" :size="14" />{{ p.name }}
        </button>
        <div v-if="playlists.length === 0" class="pb-pl-empty">还没有歌单，去左侧栏创建</div>
      </div>
    </div>

    <div class="pb-controls">
      <button class="disc-btn" title="上一首" @click="prevTrack"><PIcon name="prev" /></button>
      <button class="disc-btn main" :title="store.playing ? '暂停' : '播放'" @click="togglePlay">
        <PIcon :name="store.playing ? 'pause' : 'play'" tone="dark" />
      </button>
      <button class="disc-btn" title="下一首" @click="nextTrack"><PIcon name="next" /></button>
      <button class="disc-btn mode-btn" :title="MODE_LABEL[store.playMode]" @click="cycleMode">
        <PIcon :name="MODE_ICON[store.playMode]" />
      </button>
    </div>

    <div class="pb-progress">
      <span class="pb-time">{{ fmtTime(store.position) }}</span>
      <div class="track-groove progress" ref="progressEl" @pointerdown="seekDown">
        <div class="track-fill" :style="{ width: seekPct + '%' }" />
      </div>
      <span class="pb-time">{{ fmtTime(store.current?.duration || store.duration || 0) }}</span>
    </div>

    <div class="pb-volume">
      <button class="vol-btn" :class="{ on: volOpen }" title="音量" @pointerdown.stop="volOpen = !volOpen">
        <PIcon name="vol" :size="15" />
      </button>
      <Transition name="volpop">
        <div v-if="volOpen" class="vol-pop" @pointerdown.stop>
          <div class="vol-rail" ref="volRailEl" @pointerdown="volRailDown">
            <div class="vol-rail-fill" :style="{ height: volPct + '%' }" />
            <div class="vol-rail-thumb" :style="{ bottom: volPct + '%' }" />
          </div>
          <span class="vol-num">{{ volPct }}%</span>
        </div>
      </Transition>
    </div>

    <button class="queue-btn" :class="{ on: store.queueOpen }" title="播放队列" @click="store.queueOpen = !store.queueOpen">
      <PIcon name="songs" :size="13" />
      <span class="queue-cnt">{{ store.queue.length }}</span>
    </button>
  </footer>
</template>

<style scoped>
.player-bar {
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 10px 20px;
  background: var(--bg-2);
  border-top: 1px solid var(--line);
  flex-shrink: 0;
}
.pb-now {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 0 1 200px;
  min-width: 110px;
  min-height: 50px;
}
.pb-cover {
  width: 44px;
  height: 44px;
  border-radius: 8px;
  object-fit: cover;
  border: 1px solid var(--line);
  flex-shrink: 0;
}
.pb-cover.placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-2);
  color: var(--accent);
  font-size: 18px;
}
.pb-meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.pb-title {
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pb-artist {
  font-size: 11px;
  color: var(--text-dim);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.pb-controls {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
  padding: 0 6px;
}
.pb-progress {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  touch-action: none;
}
.pb-time {
  font-size: 11px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
  min-width: 36px;
  text-align: center;
}
.track-groove.progress {
  height: 14px;
  border-radius: 7px;
  flex: 1;
}
.track-groove.progress:hover {
  filter: brightness(1.08);
}
.pb-controls .mode-btn {
  margin-left: 4px;
}
.pb-fav {
  position: relative;
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}
.pb-fav-btn {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.6);
  background: radial-gradient(circle at 35% 30%, #4a5060, #2c3039 60%, #20242c);
  color: var(--text);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: filter 0.12s;
}
.pb-fav-btn:hover {
  filter: brightness(1.15);
}
.pb-pl-menu {
  position: absolute;
  bottom: 42px;
  left: 0;
  min-width: 190px;
  padding: 6px 0;
  background: linear-gradient(180deg, #2a2d36 0%, #23262f 100%);
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 10px;
  box-shadow: 0 10px 36px rgba(0, 0, 0, 0.55);
  z-index: 120;
}
.pb-pl-title {
  padding: 6px 14px 3px;
  font-size: 10.5px;
  color: var(--text-dim);
}
.pb-pl-empty {
  padding: 10px 14px;
  font-size: 11.5px;
  color: var(--text-dim);
}
.pb-volume {
  position: relative;
  flex-shrink: 0;
}
.vol-btn {
  width: 34px;
  height: 34px;
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 50%;
  background: radial-gradient(circle at 35% 30%, #4a5060, #2c3039 60%, #20242c);
  color: var(--text-dim);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s;
}
.vol-btn:hover {
  border-color: rgba(232, 163, 61, 0.45);
  color: var(--text);
}
.vol-btn.on {
  border-color: rgba(232, 163, 61, 0.55);
  color: var(--accent-strong);
}
.vol-pop {
  position: absolute;
  bottom: calc(100% + 12px);
  right: -4px;
  width: 42px;
  padding: 10px 0 8px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  background: linear-gradient(180deg, #2a2d36 0%, #22252d 100%);
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 10px;
  box-shadow: 0 10px 36px rgba(0, 0, 0, 0.55);
  z-index: 70;
}
.vol-rail {
  position: relative;
  width: 5px;
  height: 112px;
  border-radius: 4px;
  background: linear-gradient(180deg, #0a0b0e 0%, #1c1e24 50%, #2e3138 100%);
  border: 1px solid rgba(0, 0, 0, 0.65);
  box-shadow: inset 0 2px 5px rgba(0, 0, 0, 0.7);
  cursor: pointer;
}
.vol-rail-fill {
  position: absolute;
  left: 1px;
  right: 1px;
  bottom: 1px;
  border-radius: 3px;
  background: linear-gradient(180deg, #ffd98a, #e8a33d 50%, #b97f1e);
  box-shadow: inset 0 1px 1px rgba(255, 255, 255, 0.45);
}
.vol-rail-thumb {
  position: absolute;
  left: 50%;
  width: 13px;
  height: 13px;
  transform: translate(-50%, 50%);
  border-radius: 50%;
  background: linear-gradient(180deg, #ffffff 0%, #dfe2e8 45%, #b6bcc8 100%);
  border: 1px solid rgba(0, 0, 0, 0.45);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.5);
}
.vol-num {
  font-size: 10.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}
.volpop-enter-active,
.volpop-leave-active {
  transition: opacity 0.15s, transform 0.15s;
}
.volpop-enter-from,
.volpop-leave-to {
  opacity: 0;
  transform: translateY(5px);
}
.queue-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  padding: 0 12px;
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 17px;
  background: radial-gradient(circle at 35% 30%, #4a5060, #2c3039 60%, #20242c);
  color: var(--text-dim);
  cursor: pointer;
  font-family: inherit;
  font-size: 12px;
  flex-shrink: 0;
  transition: border-color 0.15s, color 0.15s;
}
.queue-btn:hover {
  border-color: rgba(232, 163, 61, 0.45);
  color: var(--text);
}
.queue-btn.on {
  border-color: rgba(232, 163, 61, 0.55);
  color: var(--accent-strong);
}
.queue-cnt {
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}

@media (max-width: 1100px) {
  .player-bar {
    gap: 12px;
    padding: 10px 14px;
  }
  .pb-now {
    min-width: 96px;
  }
  .pb-controls {
    gap: 8px;
    padding: 0 2px;
  }
}
@media (max-width: 900px) {
  .pb-controls .disc-btn {
    width: 34px;
    height: 34px;
  }
  .pb-now {
    min-width: 84px;
    gap: 8px;
  }
  .pb-cover {
    width: 36px;
    height: 36px;
  }
  .queue-btn {
    padding: 0 9px;
  }
}
</style>
