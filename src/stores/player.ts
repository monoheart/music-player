import { reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { PlayMode, Playlist, Song, SortDir, SortMode } from "../types";

export const store = reactive({
  songs: [] as Song[],
  queue: [] as Song[],
  current: null as Song | null,
  currentIndex: -1,
  playing: false,
  position: 0,
  duration: 0,
  volume: 0.8,
  search: "",
  view: "all" as "all" | "album" | "artist" | "fav" | "playlist" | "settings",
  sortMode: "custom" as SortMode,
  sortDir: "asc" as SortDir,
  playlists: [] as Playlist[],
  selPlaylist: null as number | null,
  libraryDir: "" as string,
  scanDirs: [] as string[],
  scanning: false,
  scrubbing: false,
  mini: false,
  nowPlaying: false,
  queueOpen: false,
  toast: "",
  lyrics: [] as { time: number; text: string }[],
  lyricIndex: -1,
  playMode: "order" as PlayMode,
  shuffleOrder: [] as number[],
  shufflePos: -1,
});

export const collator = new Intl.Collator("zh-Hans-CN", { numeric: true, sensitivity: "base" });

export function setSongs(songs: Song[]) {
  store.songs = songs;
}

export function playQueue(songs: Song[], index: number) {
  store.queue = songs;
  store.currentIndex = index;
  store.current = songs[index];
  store.playing = true;
  store.position = 0;
  if (store.playMode === "shuffle") buildShuffle();
}

function buildShuffle() {
  if (store.queue.length === 0) return;
  const indices = store.queue.map((_, i) => i);
  const cur = store.currentIndex >= 0 ? store.currentIndex : 0;
  indices.splice(indices.indexOf(cur), 1);
  for (let i = indices.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [indices[i], indices[j]] = [indices[j], indices[i]];
  }
  store.shuffleOrder = [cur, ...indices];
  store.shufflePos = 0;
}

export function setPlayMode(mode: PlayMode) {
  store.playMode = mode;
  if (mode === "shuffle") buildShuffle();
  else { store.shuffleOrder = []; store.shufflePos = -1; }
}

export function next() {
  if (store.queue.length === 0) return;
  if (store.playMode === "shuffle") {
    if (store.shuffleOrder.length === 0) buildShuffle();
    if (store.shufflePos < store.shuffleOrder.length - 1) store.shufflePos++;
    else {
      // 一轮播完：重新洗牌并跳到新顺序的第 1 位（不重播当前歌）
      buildShuffle();
      store.shufflePos = store.shuffleOrder.length > 1 ? 1 : 0;
    }
    const i = store.shuffleOrder[store.shufflePos];
    store.currentIndex = i;
    store.current = store.queue[i];
    store.position = 0;
    return;
  }
  const len = store.queue.length;
  const i = (store.currentIndex + 1) % len;
  store.currentIndex = i;
  store.current = store.queue[i];
  store.position = 0;
}

export function prev() {
  if (store.queue.length === 0) return;
  if (store.playMode === "shuffle") {
    if (store.shuffleOrder.length === 0) buildShuffle();
    if (store.shufflePos > 0) store.shufflePos--;
    else store.shufflePos = store.shuffleOrder.length - 1;
    const i = store.shuffleOrder[store.shufflePos];
    store.currentIndex = i;
    store.current = store.queue[i];
    store.position = 0;
    return;
  }
  const len = store.queue.length;
  const i = (store.currentIndex - 1 + len) % len;
  store.currentIndex = i;
  store.current = store.queue[i];
  store.position = 0;
}

export function fmtTime(sec: number): string {
  if (!isFinite(sec) || sec < 0) sec = 0;
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

export async function refreshPlaylists() {
  store.playlists = await invoke<Playlist[]>("list_playlists");
}

let toastTimer: ReturnType<typeof setTimeout> | null = null;
export function showToast(msg: string) {
  store.toast = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    store.toast = "";
  }, 1800);
}
