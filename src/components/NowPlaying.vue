<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { store, fmtTime } from "../stores/player";
import { togglePlay, seekTo, loadLyrics, nextTrack, prevTrack } from "../player";
import { MODE_ICON, MODE_LABEL, cycleMode, useSeek, useVolume, VOL_STEPS, toggleFav, usePlaylistPicker } from "../playback";
import PIcon from "./PIcon.vue";
import CoverImg from "./CoverImg.vue";
import Marquee from "./Marquee.vue";

const progressEl = ref<HTMLElement | null>(null);
const panel = ref<HTMLElement | null>(null);
const volumeEl = ref<HTMLElement | null>(null);
const lineEls: HTMLElement[] = [];
const { seekPct, seekDown } = useSeek(progressEl);
const { volPct, volDown } = useVolume(volumeEl);
const win = getCurrentWindow();

function scrollLyricTo(idx: number) {
  const el = lineEls[idx];
  if (!el || !panel.value) return;
  const ph = panel.value.clientHeight / 2;
  panel.value.scrollTo({ top: el.offsetTop - ph + el.clientHeight / 2, behavior: "smooth" });
}

watch(
  () => store.lyricIndex,
  async (idx) => {
    if (idx < 0) return;
    await nextTick();
    scrollLyricTo(idx);
  },
);

onMounted(async () => {
  if (store.current && store.lyrics.length === 0) {
    loadLyrics(store.current.path).catch(() => {});
  }
  window.addEventListener("pointerdown", closePl);
  await nextTick();
  if (store.lyricIndex >= 0) scrollLyricTo(store.lyricIndex);
});

async function winToggleMax() {
  if (await win.isMaximized()) {
    await win.unmaximize();
  } else {
    await win.maximize();
  }
}

function close() {
  store.nowPlaying = false;
}

// ---- 收藏 / 加入歌单（与 PlayerBar 共用） ----
const { open: plOpen, playlists, menuRef, toggle: togglePl, add: addToPl, closeOutside } = usePlaylistPicker();
function closePl(e: PointerEvent) {
  closeOutside(e);
}

// ---- 文件详情 ----
interface FileInfo {
  path: string;
  format: string;
  size: number;
  sample_rate: number;
  bit_rate: number;
  channels: number;
  duration: number;
}
const infoOpen = ref(false);
const fileInfo = ref<FileInfo | null>(null);
const infoErr = ref("");
let infoLoading = false;

function fmtSize(bytes: number): string {
  if (bytes >= 1024 * 1024 * 1024) return (bytes / 1024 / 1024 / 1024).toFixed(2) + " GB";
  if (bytes >= 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + " MB";
  return Math.max(1, Math.round(bytes / 1024)) + " KB";
}

const infoRows = computed(() => {
  const f = fileInfo.value;
  if (!f) return [];
  return [
    { k: "文件名", v: f.path.split(/[\\/]/).pop() ?? f.path },
    { k: "格式", v: f.format.toUpperCase() },
    { k: "文件大小", v: fmtSize(f.size) },
    { k: "时长", v: fmtTime(f.duration) },
    { k: "采样率", v: f.sample_rate ? `${(f.sample_rate / 1000).toFixed(1)} kHz` : "—" },
    { k: "比特率", v: f.bit_rate ? `${f.bit_rate} kbps` : "—" },
  ];
});

async function openInfo() {
  infoOpen.value = true;
  if (fileInfo.value || infoLoading || !store.current) return;
  infoLoading = true;
  infoErr.value = "";
  try {
    const info = await invoke<FileInfo | null>("get_file_info", { path: store.current.path });
    fileInfo.value = info;
    if (!info) infoErr.value = "无法读取文件信息";
  } catch (e) {
    infoErr.value = String(e);
  } finally {
    infoLoading = false;
  }
}
</script>

<template>
  <div class="np-overlay">
    <div class="np-drag" data-tauri-drag-region />
    <div class="np-bg-glow" />
    <button class="np-back" title="收起详情页" @click="close">
      <svg width="14" height="14" viewBox="0 0 24 24">
        <path fill="currentColor" d="M15.4 4.6 7.6 12l7.8 7.4 1.8-1.9-5.9-5.5 5.9-5.5z" />
      </svg>
      <span>收起</span>
    </button>
    <div class="np-win">
      <button class="win-btn" title="最小化" @click="win.minimize()">─</button>
      <button class="win-btn" title="最大化 / 还原" @click="winToggleMax">□</button>
      <button class="win-btn close" title="关闭" @click="win.close()">✕</button>
    </div>

    <div class="np-stage">
      <div class="np-left">
        <CoverImg v-if="store.current" :path="store.current.path" :size="380" />
        <span v-else class="np-cover"><PIcon name="note" tone="amber" :size="72" /></span>
        <Marquee class="np-title" :text="store.current?.title ?? '未在播放'" center />
        <div class="np-artist">{{ store.current?.artist ?? "—" }}</div>
        <div class="np-fav-row">
          <button
            class="np-fav-btn"
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
          <button class="np-fav-btn" title="加入歌单" @click="togglePl">
            <PIcon name="plus" :size="15" />
          </button>
          <button class="np-fav-btn" title="文件详情" @click="openInfo">
            <PIcon name="info" :size="15" />
          </button>
          <div v-if="plOpen" ref="menuRef" class="np-playlist-menu card" @click.stop>
            <div class="np-playlist-title">加入歌单</div>
            <button v-for="p in playlists" :key="p.id" class="ctx-item" @click="addToPl(store.current, p.id)">
              <PIcon name="list" :size="14" />{{ p.name }}
            </button>
            <div v-if="playlists.length === 0" class="np-playlist-empty">还没有歌单，先去左侧栏创建</div>
          </div>
        </div>
        <div class="np-controls">
          <button class="disc-btn" title="上一首" @click="prevTrack"><PIcon name="prev" /></button>
          <button class="disc-btn main" :title="store.playing ? '暂停' : '播放'" @click="togglePlay">
            <PIcon :name="store.playing ? 'pause' : 'play'" tone="dark" />
          </button>
          <button class="disc-btn" title="下一首" @click="nextTrack"><PIcon name="next" /></button>
          <button class="disc-btn mode-btn" :title="MODE_LABEL[store.playMode]" @click="cycleMode">
            <PIcon :name="MODE_ICON[store.playMode]" />
          </button>
        </div>
      </div>
      <div class="np-right">
        <div ref="panel" class="np-lyric-scroll">
          <div class="np-lyric-inner">
            <template v-if="store.lyrics.length > 0">
              <p
                v-for="(l, i) in store.lyrics"
                :key="i"
                :ref="(el) => { if (el) lineEls[i] = el as HTMLElement; }"
                class="np-line"
                :class="{ active: i === store.lyricIndex }"
                @click="seekTo(l.time)"
              >
                {{ l.text }}
              </p>
            </template>
            <p v-else class="np-line none">暂无歌词</p>
          </div>
        </div>
      </div>
    </div>

    <div class="np-bottom">
      <div class="np-progress">
        <span class="np-time">{{ fmtTime(store.position) }}</span>
        <div class="track-groove progress" ref="progressEl" @pointerdown="seekDown">
          <div class="track-fill" :style="{ width: seekPct + '%' }" />
        </div>
        <span class="np-time">{{ fmtTime(store.current?.duration || store.duration || 0) }}</span>
      </div>

      <div class="np-volume">
        <span class="vol-icon"><PIcon name="vol" /></span>
        <div class="vol-track" ref="volumeEl" @pointerdown="volDown">
          <div class="vol-fill" :style="{ width: volPct + '%' }" />
          <div class="vol-steps">
            <span v-for="s in VOL_STEPS" :key="s" class="vol-step" />
          </div>
          <div class="vol-thumb" :style="{ left: volPct + '%' }" />
        </div>
        <span class="np-time vol-num">{{ volPct }}%</span>
      </div>
    </div>

    <Transition name="qmask">
      <div v-if="infoOpen" class="np-info-mask" @click="infoOpen = false">
        <div class="np-info-panel card" @click.stop>
          <div class="np-info-head">
            <span class="np-info-title">文件详情</span>
            <button class="np-info-close" title="关闭" @click="infoOpen = false">✕</button>
          </div>
          <div class="np-info-song">{{ store.current?.title }}</div>
          <div v-if="fileInfo" class="np-info-grid">
            <div v-for="row in infoRows" :key="row.k" class="np-info-row">
              <span class="np-info-k">{{ row.k }}</span>
              <span class="np-info-v">{{ row.v }}</span>
            </div>
          </div>
          <div v-else class="np-info-loading">{{ infoErr || "读取中…" }}</div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.np-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(180deg, #262a31 0%, #1e2127 100%);
  overflow: hidden;
}
.np-bg-glow {
  position: absolute;
  width: 70%;
  height: 70%;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: radial-gradient(ellipse at center, rgba(232, 163, 61, 0.12), transparent 65%);
  pointer-events: none;
}
/* 顶部拖拽区：在 np-back/np-win（z-index 5）之下，空白处可拖动窗口 */
.np-drag {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 44px;
  z-index: 4;
}
.np-back {
  position: absolute;
  top: 14px;
  left: 14px;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border-radius: 20px;
  border: 1px solid rgba(0, 0, 0, 0.5);
  background: linear-gradient(180deg, #4a4f5a 0%, #34373f 100%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.15);
  color: #cfc7b4;
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  z-index: 5;
}
.np-back:hover {
  filter: brightness(1.2);
}
.np-win {
  position: absolute;
  top: 14px;
  right: 14px;
  display: flex;
  gap: 6px;
  z-index: 5;
}
.np-win .win-btn {
  width: 32px;
  height: 30px;
  border-radius: 16px;
  border: 1px solid rgba(0, 0, 0, 0.5);
  background: linear-gradient(180deg, #4a4f5a 0%, #34373f 100%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.15);
  color: #cfc7b4;
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
}
.np-win .win-btn:hover {
  filter: brightness(1.2);
}
.np-win .win-btn.close:hover {
  background: linear-gradient(180deg, #e06c5a 0%, #b8483a 100%);
  color: #fff;
}
.np-stage {
  display: flex;
  gap: 80px;
  align-items: center;
  justify-content: center;
  max-width: 1180px;
  width: 100%;
  padding: 70px 60px 128px;
}
.np-left {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  flex-shrink: 0;
}
.np-cover {
  width: 380px;
  height: 380px;
  border-radius: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, var(--accent-strong), var(--accent));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.3), 0 14px 40px rgba(0, 0, 0, 0.5);
}
.np-title {
  font-size: 22px;
  font-weight: 700;
  color: #f5e3bc;
  max-width: 380px;
  width: 380px;
}
.np-artist {
  font-size: 15px;
  color: var(--text-dim);
  max-width: 380px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.np-controls {
  display: flex;
  align-items: center;
  gap: 18px;
  margin-top: 8px;
}
.np-controls .disc-btn.main {
  width: 56px;
  height: 56px;
}
.np-controls .disc-btn.main svg {
  width: 24px;
  height: 24px;
}
.np-fav-row {
  position: relative;
  display: flex;
  gap: 10px;
}
.np-fav-btn {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.6);
  background: radial-gradient(circle at 35% 30%, #4a5060, #2c3039 60%, #20242c);
  color: var(--text);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: opacity 0.15s;
}
.np-fav-btn:hover {
  filter: brightness(1.15);
}
.np-playlist-menu {
  position: absolute;
  top: 40px;
  right: 0;
  min-width: 190px;
  padding: 6px 0;
  background: linear-gradient(180deg, #2a2d36 0%, #23262f 100%);
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 10px;
  box-shadow: 0 10px 36px rgba(0, 0, 0, 0.55);
  z-index: 20;
}
.np-playlist-title {
  padding: 6px 14px 3px;
  font-size: 10.5px;
  color: var(--text-dim);
}
.np-playlist-empty {
  padding: 10px 14px;
  font-size: 11.5px;
  color: var(--text-dim);
}
.np-right {
  flex: 1;
  min-width: 0;
  max-width: 500px;
  height: min(520px, 60vh);
}
.np-lyric-scroll {
  height: 100%;
  overflow-y: auto;
  min-height: 0;
  mask-image: linear-gradient(180deg, transparent 0, #000 10%, #000 90%, transparent 100%);
}
.np-lyric-inner {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 22px;
  padding: 200px 0;
}
.np-line {
  margin: 0;
  font-size: 17px;
  line-height: 1.6;
  text-align: center;
  color: #9a9589;
  transition: font-size 0.25s, color 0.25s, transform 0.25s, opacity 0.25s;
  cursor: pointer;
}
.np-line:hover {
  color: #c9c2b0;
}
.np-line.active {
  font-size: 21px;
  font-weight: 700;
  color: #f5e3bc;
  transform: translateY(-2px);
  text-shadow: 0 1px 8px rgba(232, 163, 61, 0.35);
}
.np-line.active:hover {
  color: #ffe9bd;
}
.np-line.none {
  color: var(--text-dim);
  padding-top: 40px;
  cursor: default;
}
.np-bottom {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 92px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 26px;
  padding: 0 40px;
}
.np-progress {
  display: flex;
  align-items: center;
  gap: 14px;
  flex: 0 1 680px;
  min-width: 0;
  touch-action: none;
}
.np-progress .track-groove.progress {
  flex: 1;
  min-width: 0;
}
.np-time {
  font-size: 11px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  min-width: 34px;
  text-align: center;
  flex-shrink: 0;
}
.np-volume {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  touch-action: none;
}
.vol-icon {
  font-size: 14px;
  color: var(--text-dim);
  flex-shrink: 0;
  display: flex;
}
.vol-track {
  height: 14px;
  width: 96px;
  position: relative;
  border-radius: 7px;
  background: linear-gradient(180deg, #0a0b0e 0%, #1c1e24 45%, #2e3138 100%);
  border: 1px solid rgba(0, 0, 0, 0.65);
  box-shadow: inset 0 2px 5px rgba(0, 0, 0, 0.7), inset 0 -1px 1px rgba(255, 255, 255, 0.08),
    0 1px 0 rgba(255, 255, 255, 0.06);
  cursor: pointer;
  overflow: visible;
}
.vol-fill {
  position: absolute;
  left: 1px;
  top: 2px;
  bottom: 2px;
  border-radius: 5px;
  background: linear-gradient(180deg, #f7c46b, #e8a33d 55%, #c8872a);
  box-shadow: inset 0 1px 1px rgba(255, 255, 255, 0.3);
}
.vol-steps {
  position: absolute;
  inset: 0;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 5px;
}
.vol-step {
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.18);
}
.vol-thumb {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: linear-gradient(180deg, #ffffff 0%, #dfe2e8 45%, #b6bcc8 100%);
  border: 1px solid rgba(0, 0, 0, 0.6);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.6);
  pointer-events: none;
}
.vol-num {
  min-width: 38px;
  flex-shrink: 0;
}

.np-info-mask {
  position: fixed;
  inset: 0;
  z-index: 60;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
}
.np-info-panel {
  width: 360px;
  max-width: 90vw;
  background: linear-gradient(180deg, #2a2d36 0%, #22252d 100%);
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 12px;
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.6);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.np-info-head {
  display: flex;
  align-items: center;
  gap: 10px;
}
.np-info-title {
  flex: 1;
  font-size: 14px;
  font-weight: 700;
  color: #f5e3bc;
}
.np-info-close {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.5);
  background: linear-gradient(180deg, #4a4f5a 0%, #34373f 100%);
  color: var(--text-dim);
  font-size: 11px;
  line-height: 1;
  cursor: pointer;
  flex-shrink: 0;
}
.np-info-close:hover {
  filter: brightness(1.2);
}
.np-info-song {
  font-size: 12px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.np-info-grid {
  display: flex;
  flex-direction: column;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.18);
  padding: 4px 0;
}
.np-info-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 12px;
}
.np-info-k {
  width: 68px;
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-dim);
}
.np-info-v {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.np-info-loading {
  padding: 20px 0;
  text-align: center;
  font-size: 12px;
  color: var(--text-dim);
}

/* 窄窗口（含最小窗口 1000px 宽度）：收紧横向布局 */
@media (max-width: 1100px) {
  .np-stage {
    gap: 40px;
    padding: 50px 30px 130px;
  }
  .np-bottom {
    padding: 0 20px;
    gap: 16px;
  }
  .np-progress {
    flex-basis: 420px;
  }
  .np-title,
  .np-artist {
    max-width: 280px;
    width: 280px;
  }
}
/* 中等高度：封面缩小，保证内容不溢出窗口 */
@media (max-height: 860px) {
  .np-stage {
    padding: 70px 30px 130px;
  }
  .np-left :deep(.cover-img),
  .np-cover {
    width: 250px !important;
    height: 250px !important;
  }
  .np-right {
    height: min(400px, 52vh);
  }
}
/* 矮窗口（最小高度 680px 附近）：进一步缩小，避免底部栏与内容重叠 */
@media (max-height: 700px) {
  .np-left :deep(.cover-img),
  .np-cover {
    width: 180px !important;
    height: 180px !important;
  }
  .np-right {
    height: min(300px, 42vh);
  }
  .np-title {
    font-size: 19px;
  }
}
</style>