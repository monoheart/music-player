<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize, LogicalPosition } from "@tauri-apps/api/window";
import { store, setSongs, setPlayMode, playQueue, refreshPlaylists, showToast, collator, fmtTime } from "./stores/player";
import { play, togglePlay, toggleMini, nextTrack, prevTrack } from "./player";
import { toggleFav } from "./playback";
import type { LastState, PlayMode, Song, Playlist } from "./types";
import { SORT_OPTIONS } from "./types";
import PlayerBar from "./components/PlayerBar.vue";
import PIcon from "./components/PIcon.vue";
import SongTable from "./components/SongTable.vue";
import NowPlaying from "./components/NowPlaying.vue";
import CoverImg from "./components/CoverImg.vue";
import QueuePanel from "./components/QueuePanel.vue";
import Marquee from "./components/Marquee.vue";

const errorMsg = ref("");
const selAlbum = ref("");
const selArtist = ref("");
// 用户是否手动拖过全部歌曲的自定义顺序（拖过后新扫描的歌自动排末尾）
let libraryCustomized = false;
function sortByTitle(songs: Song[]): Song[] {
  return [...songs].sort((a, b) => collator.compare(a.title, b.title) || collator.compare(a.artist, b.artist));
}

// ---- 当前可见歌曲列表（右键播放应与双击行为一致） ----
const visibleSongs = computed(() => {
  if (store.view === "album") {
    if (store.search.trim()) return filtered.value;
    if (selAlbum.value) return albumGroups.value.find((g) => g.name === selAlbum.value)?.songs ?? [];
  }
  if (store.view === "artist") {
    if (store.search.trim()) return filtered.value;
    if (selArtist.value) return artistGroups.value.find((g) => g.name === selArtist.value)?.songs ?? [];
  }
  if (store.view === "playlist" && store.selPlaylist !== null) {
    return plShown.value;
  }
  return filtered.value;
});

// ---- 右键菜单 ----
const ctxMenu = ref<{ x: number; y: number; song: Song; index: number } | null>(null);
function openCtx(e: MouseEvent, song: Song, index: number) {
  e.preventDefault();
  ctxMenu.value = { x: e.clientX, y: e.clientY, song, index };
}
function closeCtx() {
  ctxMenu.value = null;
}
function ctxPlay() {
  if (!ctxMenu.value) return;
  const { song } = ctxMenu.value;
  const list = visibleSongs.value;
  const i = list.findIndex((s) => s.path === song.path);
  if (i >= 0) {
    playQueue(list.slice(), i);
    play(song.path).catch(() => {});
  }
  closeCtx();
}
function ctxPlayNext() {
  if (!ctxMenu.value) return;
  const { song } = ctxMenu.value;
  if (store.queue.length === 0) store.queue = store.songs.slice();
  const fromIdx = store.queue.findIndex((s) => s.path === song.path);
  if (fromIdx >= 0 && store.currentIndex >= 0 && fromIdx !== store.currentIndex + 1) {
    const [m] = store.queue.splice(fromIdx, 1);
    const insertAt = fromIdx < store.currentIndex ? store.currentIndex : store.currentIndex + 1;
    store.queue.splice(insertAt, 0, m);
    if (fromIdx < store.currentIndex) store.currentIndex--;
  } else if (fromIdx < 0) {
    const insertAt = store.currentIndex >= 0 ? store.currentIndex + 1 : 0;
    store.queue.splice(insertAt, 0, song);
  }
  if (store.playMode === "shuffle") setPlayMode("shuffle");
  closeCtx();
}
async function ctxToggleFav() {
  if (!ctxMenu.value) return;
  await toggleFav(ctxMenu.value.song);
  closeCtx();
}
function ctxReveal() {
  if (!ctxMenu.value) return;
  invoke("reveal_in_folder", { path: ctxMenu.value.song.path }).catch(() => {});
  closeCtx();
}
function ctxRemoveFromQueue() {
  if (!ctxMenu.value) return;
  const { song } = ctxMenu.value;
  const i = store.queue.findIndex((s) => s.path === song.path);
  if (i < 0) { closeCtx(); return; }
  store.queue.splice(i, 1);
  // 移除的排在当前之前（含当前正在播的）→ 当前索引前移，保证播完下一首正确
  if (i <= store.currentIndex) store.currentIndex--;
  if (store.currentIndex >= store.queue.length) store.currentIndex = store.queue.length - 1;
  if (store.playMode === "shuffle") setPlayMode("shuffle");
  closeCtx();
}

function onKeydown(e: KeyboardEvent) {
  // 迷你模式只做后台播放，不响应任何全局快捷键（空格暂停等），避免与游戏冲突
  if (store.mini) return;
  const t = e.target as HTMLElement | null;
  if (t && (t.tagName === "INPUT" || t.tagName === "SELECT" || t.tagName === "TEXTAREA")) return;
  if (e.key === "Escape") {
    if (store.nowPlaying) store.nowPlaying = false;
    if (store.queueOpen) store.queueOpen = false;
    return;
  }
  // Ctrl+←/→ 切歌（preventDefault 阻止 WebView2 的浏览器前进/后退）
  if (e.ctrlKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
    e.preventDefault();
    if (e.key === "ArrowLeft") prevTrack();
    else nextTrack();
    return;
  }
  if (e.code !== "Space") return;
  // 按钮聚焦时交给按钮自身，避免双重触发
  if (t && t.tagName === "BUTTON") return;
  e.preventDefault();
  togglePlay().catch(() => {});
}

onMounted(async () => {
  try {
    const songs = await invoke<Song[]>("get_songs");
    const last = await invoke<LastState | null>("get_last_state");
    libraryCustomized = last?.custom_sort ?? false;
    // 默认自定义排序：从未拖过时按标题（拼音）展示，拖过后按保存的顺序
    if (songs.length) setSongs(libraryCustomized ? songs : sortByTitle(songs));
    store.playlists = await invoke<Playlist[]>("list_playlists");
    const dirs = await invoke<string[]>("get_scan_dirs");
    store.scanDirs = dirs;
    store.libraryDir = dirs[0] ?? "";
    if (last) {
      if (last.volume >= 0 && last.volume <= 1) store.volume = last.volume;
      if (["order", "shuffle", "repeat"].includes(last.play_mode)) {
        setPlayMode(last.play_mode as PlayMode);
      }
      // 每次打开固定为「自定义」排序
      store.sortMode = "custom";
      if (last.sort_dir === "asc" || last.sort_dir === "desc") {
        store.sortDir = last.sort_dir;
      }
      if (last.song_path) {
        const cur = songs.find((s) => s.path === last.song_path);
        if (cur) {
          store.current = cur;
          store.position = last.position;
          if (last.queue.length) {
            // 按保存的原始顺序恢复队列（拖拽顺序不丢失）
            const byPath = new Map(songs.map((s) => [s.path, s]));
            store.queue = last.queue.map((p) => byPath.get(p)).filter((s): s is Song => !!s);
            store.currentIndex = store.queue.findIndex((s) => s.path === last.song_path);
            if (store.playMode === "shuffle") setPlayMode("shuffle");
          }
        }
      }
    }
  } catch {
    /* 恢复失败不影响使用 */
  }
  window.addEventListener("keydown", onKeydown);
  await restoreWindowState();
  win.onMoved(scheduleWinSave);
  win.onResized(scheduleWinSave);
  win.setMinSize(new LogicalSize(1100, 720)).catch(() => {});
});

async function addDir() {
  try {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir !== "string" || !dir) return;
    await invoke("add_scan_dir", { path: dir });
    store.scanDirs.push(dir);
    store.libraryDir = dir;
    await scanAll();
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function removeDir(dir: string) {
  try {
    await invoke("remove_scan_dir", { path: dir });
    store.scanDirs = store.scanDirs.filter((d) => d !== dir);
    store.libraryDir = store.scanDirs[0] ?? "";
    await scanAll();
  } catch (e) {
    errorMsg.value = String(e);
  }
}

async function scanAll() {
  if (store.scanning) return;
  store.scanning = true;
  errorMsg.value = "";
  try {
    const all: Song[] = [];
    for (const dir of store.scanDirs) {
      const songs = await invoke<Song[]>("scan_library", { dir });
      all.push(...songs);
    }
    setSongs(libraryCustomized ? all : sortByTitle(all));
  } catch (e) {
    errorMsg.value = String(e);
  } finally {
    store.scanning = false;
  }
}

const sortedSongs = computed(() => {
  if (store.sortMode === "custom") return store.songs;
  const dir = store.sortDir === "desc" ? -1 : 1;
  const c = collator;
  const arr = [...store.songs];
  const pathCmp = (a: Song, b: Song) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  arr.sort((a, b) => {
    let r: number;
    if (store.sortMode === "artist") r = c.compare(a.artist, b.artist) || c.compare(a.title, b.title) || pathCmp(a, b);
    else if (store.sortMode === "album") r = c.compare(a.album, b.album) || c.compare(a.title, b.title) || pathCmp(a, b);
    else r = c.compare(a.title, b.title) || c.compare(a.artist, b.artist) || pathCmp(a, b);
    return r * dir;
  });
  return arr;
});

const filtered = computed(() => {
  const q = store.search.trim().toLowerCase();
  const base = store.view === "fav" ? sortedSongs.value.filter((s) => s.fav) : sortedSongs.value;
  if (!q) return base;
  return base.filter(
    (s) =>
      s.title.toLowerCase().includes(q) ||
      s.artist.toLowerCase().includes(q) ||
      s.album.toLowerCase().includes(q),
  );
});

const albumGroups = computed(() => {
  const m = new Map<string, Song[]>();
  for (const s of store.songs) {
    const k = s.album;
    if (!m.has(k)) m.set(k, []);
    m.get(k)!.push(s);
  }
  const groups = [...m.entries()].map(([name, songs]) => {
    const allTracks = songs.every((s) => (s.track ?? 0) > 0);
    songs.sort(
      allTracks
        ? (a, b) => (a.track! - b.track!) || collator.compare(a.title, b.title)
        : (a, b) => collator.compare(a.title, b.title),
    );
    return { name, songs };
  });
  groups.sort((a, b) => collator.compare(a.name, b.name));
  return groups;
});

const artistGroups = computed(() => {
  const m = new Map<string, Song[]>();
  for (const s of store.songs) {
    const k = s.artist;
    if (!m.has(k)) m.set(k, []);
    m.get(k)!.push(s);
  }
  const groups = [...m.entries()].map(([name, songs]) => {
    songs.sort((a, b) => collator.compare(a.title, b.title));
    return { name, songs };
  });
  groups.sort((a, b) => collator.compare(a.name, b.name));
  return groups;
});

const favCount = computed(() => store.songs.filter((s) => s.fav).length);

function switchView(v: "all" | "album" | "artist" | "fav" | "playlist" | "settings") {
  store.view = v;
  selAlbum.value = "";
  selArtist.value = "";
  if (v !== "playlist") {
    store.selPlaylist = null;
    playlistSongs.value = [];
  }
}

// ---- 歌单 ----
const playlistSongs = ref<Song[]>([]);
const newPlaylistName = ref("");
const renamingId = ref<number | null>(null);
const renamingName = ref("");
const plNewOpen = ref(false);
const curPlaylist = computed(() =>
  store.selPlaylist === null ? null : store.playlists.find((p) => p.id === store.selPlaylist) ?? null,
);

function openPlaylist(p: Playlist) {
  store.view = "playlist";
  loadPlaylistSongs(p.id);
}
function openAndPlay(p: Playlist) {
  openPlaylist(p);
  playPlaylist(p);
}
async function loadPlaylistSongs(id: number) {
  store.selPlaylist = id;
  playlistSongs.value = await invoke<Song[]>("get_playlist_songs", { id });
}
async function createPlaylistAndOpen() {
  const name = newPlaylistName.value.trim();
  if (!name) {
    plNewOpen.value = false;
    return;
  }
  try {
    const id = await invoke<number>("create_playlist", { name });
    newPlaylistName.value = "";
    plNewOpen.value = false;
    await refreshPlaylists();
    store.view = "playlist";
    await loadPlaylistSongs(id);
  } catch (e) {
    errorMsg.value = String(e);
  }
}
async function renamePlaylist(id: number) {
  const name = renamingName.value.trim();
  if (!name) return;
  try {
    await invoke("rename_playlist", { id, name });
    renamingId.value = null;
    await refreshPlaylists();
  } catch (e) {
    errorMsg.value = String(e);
  }
}
async function playPlaylist(p: Playlist) {
  const songs = await invoke<Song[]>("get_playlist_songs", { id: p.id });
  if (songs.length === 0) return;
  playQueue(songs, 0);
  play(songs[0].path).catch(() => {});
}

// ---- 歌单内歌曲（含搜索过滤） ----
const plShown = computed(() => {
  const q = store.search.trim().toLowerCase();
  if (!q) return playlistSongs.value;
  return playlistSongs.value.filter(
    (s) =>
      s.title.toLowerCase().includes(q) ||
      s.artist.toLowerCase().includes(q) ||
      s.album.toLowerCase().includes(q),
  );
});

// ---- 批量添加歌曲 ----
const plAddOpen = ref(false);
const plAddQ = ref("");
const plAddSel = ref<string[]>([]);
const plAddCandidates = computed(() => {
  const inPl = new Set(playlistSongs.value.map((s) => s.path));
  const q = plAddQ.value.trim().toLowerCase();
  return store.songs.filter(
    (s) =>
      !inPl.has(s.path) &&
      (!q ||
        s.title.toLowerCase().includes(q) ||
        s.artist.toLowerCase().includes(q) ||
        s.album.toLowerCase().includes(q)),
  );
});
function plAddToggle(path: string) {
  const i = plAddSel.value.indexOf(path);
  if (i >= 0) plAddSel.value.splice(i, 1);
  else plAddSel.value.push(path);
}
function plAddToggleAll() {
  const paths = plAddCandidates.value.map((s) => s.path);
  const all = paths.every((p) => plAddSel.value.includes(p));
  plAddSel.value = all ? [] : paths;
}
function plAddOpenDialog() {
  plAddQ.value = "";
  plAddSel.value = [];
  plAddOpen.value = true;
}
async function plAddConfirm() {
  const paths = plAddSel.value;
  if (paths.length === 0 || store.selPlaylist === null) return;
  try {
    await invoke("add_songs_to_playlist", { id: store.selPlaylist, paths });
    await loadPlaylistSongs(store.selPlaylist);
    await refreshPlaylists();
    plAddOpen.value = false;
    showToast(`已添加 ${paths.length} 首歌曲`);
  } catch (e) {
    errorMsg.value = String(e);
  }
}
function onReorder(arr: Song[]) {
  if (store.view === "playlist" && store.selPlaylist !== null) {
    playlistSongs.value = arr;
    invoke("reorder_playlist_songs", { id: store.selPlaylist, paths: arr.map((s) => s.path) }).catch(() => {});
  } else if (store.view === "all") {
    store.songs = arr;
    libraryCustomized = true;
    invoke("reorder_songs", { paths: arr.map((s) => s.path) }).catch(() => {});
  }
}
// 侧栏歌单按下即拖动排序
const plOverId = ref<number | null>(null);
let plDrag: {
  id: number;
  moving: boolean;
  startX: number;
  startY: number;
  grabOffset: number;
} | null = null;
let plSuppressClick = false;
const PL_MOVE_TOL = 4;
function plPointerDown(e: PointerEvent, p: Playlist) {
  if (e.button !== 0) return;
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
  plDrag = { id: p.id, moving: false, startX: e.clientX, startY: e.clientY, grabOffset: e.clientY - rect.top };
  window.addEventListener("pointermove", plPointerMove);
  window.addEventListener("pointerup", plPointerUp);
  window.addEventListener("pointercancel", plDragCancel);
}
function plPointerMove(e: PointerEvent) {
  if (!plDrag) return;
  if (!plDrag.moving) {
    if (Math.abs(e.clientX - plDrag.startX) <= PL_MOVE_TOL && Math.abs(e.clientY - plDrag.startY) <= PL_MOVE_TOL) {
      return;
    }
    plDrag.moving = true;
    plOverId.value = plDrag.id;
    plSuppressClick = true;
    setTimeout(() => {
      plSuppressClick = false;
    }, 500);
    document.body.style.userSelect = "none";
  }
  const list = document.querySelector<HTMLElement>(".pl-section");
  if (!list) return;
  const rows = Array.from(list.querySelectorAll<HTMLElement>(".pl-row[data-pid]"));
  const from = store.playlists.findIndex((p) => p.id === plDrag!.id);
  if (from < 0) return;
  // 被拖行的顶部跟随鼠标（保持按下时在行内的相对位置），据此找插入位
  const currentTop = e.clientY - plDrag.grabOffset;
  let target = -1;
  let vis = -1;
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i];
    if (Number(r.dataset.pid) === plDrag.id) continue;
    vis++;
    const rc = r.getBoundingClientRect();
    if (currentTop < rc.top + rc.height / 2) {
      target = vis;
      break;
    }
  }
  if (target < 0) target = vis + 1;
  if (target !== from) {
    const arr = store.playlists.slice();
    const [m] = arr.splice(from, 1);
    arr.splice(target, 0, m);
    store.playlists = arr;
    plDrag.id = arr[target].id;
    plOverId.value = plDrag.id;
  }
  const r = list.getBoundingClientRect();
  if (e.clientY < r.top + 30) list.scrollTop -= 14;
  else if (e.clientY > r.bottom - 30) list.scrollTop += 14;
}
function plPointerUp() {
  if (!plDrag) return;
  const wasMoving = plDrag.moving;
  plDragCancel();
  if (wasMoving) {
    invoke("reorder_playlists", { ids: store.playlists.map((p) => p.id) }).catch(() => {});
  }
}
function plDragCancel() {
  if (!plDrag) return;
  window.removeEventListener("pointermove", plPointerMove);
  window.removeEventListener("pointerup", plPointerUp);
  window.removeEventListener("pointercancel", plDragCancel);
  plDrag = null;
  plOverId.value = null;
  document.body.style.userSelect = "";
}
function plRowClick(p: Playlist) {
  if (plSuppressClick) return;
  openPlaylist(p);
}
async function ctxAddToPlaylist(id: number) {
  const song = plPickSong.value;
  if (!song) return;
  const name = store.playlists.find((p) => p.id === id)?.name ?? "歌单";
  await invoke("add_to_playlist", { id, path: song.path });
  if (store.selPlaylist === id) await loadPlaylistSongs(id);
  await refreshPlaylists();
  showToast(`已加入「${name}」`);
  plPickOpen.value = false;
}
const plPickOpen = ref(false);
const plPickSong = ref<Song | null>(null);
function ctxOpenAddToPl() {
  if (!ctxMenu.value) return;
  plPickSong.value = ctxMenu.value.song;
  plPickOpen.value = true;
  closeCtx();
}
async function ctxRemoveFromPlaylist(id: number) {
  if (!ctxMenu.value) return;
  await invoke("remove_from_playlist", { id, path: ctxMenu.value.song.path });
  await loadPlaylistSongs(id);
  await refreshPlaylists();
  closeCtx();
}

async function playGroup(songs: Song[]) {
  if (songs.length === 0) return;
  playQueue(songs.slice(), 0);
  play(songs[0].path).catch(() => {});
}

async function miniToggleFav() {
  if (!store.current) return;
  await toggleFav(store.current);
}
function miniDragStart(e: PointerEvent) {
  const t = e.target as HTMLElement;
  if (t.closest("button")) return;
  win.startDragging();
}

const win = getCurrentWindow();

// ---- 窗口位置/大小记忆 ----
let winSaveTimer: ReturnType<typeof setTimeout> | null = null;
async function saveWindowState() {
  if (store.mini || store.nowPlaying) return;
  try {
    const p = await win.outerPosition();
    const s = await win.outerSize();
    await invoke("set_window_state", {
      state: JSON.stringify({ x: p.x, y: p.y, w: s.width, h: s.height }),
    });
  } catch { /* */ }
}
function scheduleWinSave() {
  if (winSaveTimer) clearTimeout(winSaveTimer);
  winSaveTimer = setTimeout(saveWindowState, 600);
}
async function restoreWindowState() {
  try {
    const raw = await invoke<string>("get_window_state");
    const s = JSON.parse(raw) as { x?: number; y?: number; w?: number; h?: number };
    if (typeof s.w === "number" && typeof s.h === "number") {
      await win.setSize(new LogicalSize(Math.max(1100, Math.min(s.w, 4000)), Math.max(720, Math.min(s.h, 3000))));
    }
    if (typeof s.x === "number" && typeof s.y === "number") {
      await win.setPosition(new LogicalPosition(s.x, s.y));
    }
  } catch { /* */ }
}

async function winMin() {
  await win.minimize();
}
async function winToggleMax() {
  if (await win.isMaximized()) {
    await win.unmaximize();
  } else {
    await win.maximize();
  }
}
function winClose() {
  win.close();
}
</script>

<template>
  <div v-if="store.mini" class="mini-app" @pointerdown="miniDragStart">
    <div class="mini-bar">
      <CoverImg v-if="store.current" :path="store.current.path" :size="52" />
      <span v-else class="mini-cover"><PIcon name="note" tone="amber" :size="22" /></span>
      <div class="mini-meta">
        <Marquee class="mini-title" :text="store.current?.title ?? '未在播放'" />
        <div class="mini-artist">{{ store.current?.artist || store.current?.album || "—" }}</div>
      </div>
      <div class="mini-controls">
        <button class="disc-btn small" title="上一首" @click="prevTrack"><PIcon name="prev" :size="12" /></button>
        <button class="disc-btn small main" :title="store.playing ? '暂停' : '播放'" @click="togglePlay">
          <PIcon :name="store.playing ? 'pause' : 'play'" tone="dark" :size="13" />
        </button>
        <button class="disc-btn small" title="下一首" @click="nextTrack"><PIcon name="next" :size="12" /></button>
      </div>
      <button
        class="mini-fav"
        :class="{ on: store.current?.fav }"
        :title="store.current?.fav ? '取消收藏' : '收藏'"
        @click="miniToggleFav"
      >
        <PIcon
          :name="store.current?.fav ? 'heart' : 'heart-o'"
          :tone="store.current?.fav ? 'amber' : 'light'"
          :size="14"
        />
      </button>
      <button class="mini-exit" title="退出迷你模式" @click="toggleMini">×</button>
    </div>
  </div>
  <div v-else class="app">
    <header class="toolbar" data-tauri-drag-region @dblclick="winToggleMax">
      <div class="brand">
        <span class="logo"><PIcon name="note" /></span>
        <div>
          <h1>音乐播放器</h1>
        </div>
      </div>
      <div class="toolbar-right">
        <div class="search-box">
          <span class="search-icon"><PIcon name="search" /></span>
          <input v-model="store.search" type="text" placeholder="搜索歌曲 / 歌手 / 专辑" @keydown.esc="store.search = ''" />
        </div>
        <div class="win-btns">
          <button class="win-btn" title="迷你模式" @click="toggleMini"><PIcon name="mini" :size="12" /></button>
          <button class="win-btn" title="最小化" @click="winMin">─</button>
          <button class="win-btn" title="最大化 / 还原" @click="winToggleMax">□</button>
          <button class="win-btn close" title="关闭" @click="winClose">✕</button>
        </div>
      </div>
    </header>

    <div class="body">
      <aside class="sidebar card">
        <button class="nav-btn" :class="{ active: store.view === 'all' }" @click="switchView('all')">
          <PIcon name="songs" :size="15" />
          <span class="nav-label">全部歌曲</span>
          <span class="count">{{ store.songs.length }}</span>
        </button>
        <button class="nav-btn" :class="{ active: store.view === 'album' }" @click="switchView('album')">
          <PIcon name="album" :size="15" />
          <span class="nav-label">专辑</span>
          <span class="count">{{ albumGroups.length }}</span>
        </button>
        <button class="nav-btn" :class="{ active: store.view === 'artist' }" @click="switchView('artist')">
          <PIcon name="artist" :size="15" />
          <span class="nav-label">歌手</span>
          <span class="count">{{ artistGroups.length }}</span>
        </button>
        <div class="pl-section">
          <div class="pl-head">
            <span class="pl-title">歌单</span>
            <span class="count">{{ store.playlists.length }}</span>
          </div>
          <button
            class="nav-btn pl-row"
            :class="{ active: store.view === 'fav' }"
            title="收藏"
            @click="switchView('fav')"
          >
            <PIcon :name="store.view === 'fav' ? 'heart' : 'heart-o'" :size="15" :tone="store.view === 'fav' ? 'amber' : 'light'" />
            <span class="nav-label pl-name">收藏</span>
          </button>
          <div
            v-for="p in store.playlists"
            :key="p.id"
            class="nav-btn pl-row"
            :class="{ active: store.view === 'playlist' && store.selPlaylist === p.id, 'drag-over': plOverId === p.id }"
            :title="p.name"
            :data-pid="p.id"
            @click="plRowClick(p)"
            @dblclick="openAndPlay(p)"
            @pointerdown="plPointerDown($event, p)"
          >
            <PIcon name="note" :size="13" :tone="store.selPlaylist === p.id && store.view === 'playlist' ? 'amber' : 'light'" />
            <span class="nav-label pl-name">{{ p.name }}</span>
          </div>
          <div v-if="plNewOpen" class="pl-new">
            <input
              v-model="newPlaylistName"
              class="input"
              placeholder="歌单名称，回车创建"
              @keydown.enter="createPlaylistAndOpen"
              @keydown.esc="plNewOpen = false"
            />
          </div>
          <button class="nav-btn pl-new-btn" @click="plNewOpen = true">＋ 新建歌单</button>
        </div>
        <div class="sidebar-bottom">
          <button class="nav-btn settings-btn" :class="{ active: store.view === 'settings' }" @click="switchView('settings')">
            <svg width="15" height="15" viewBox="0 0 24 24">
              <path
                fill="currentColor"
                d="M12 8.4a3.6 3.6 0 1 0 0 7.2 3.6 3.6 0 0 0 0-7.2zm9.2 3.6a7.2 7.2 0 0 0-.12-1.3l2-1.55-2-3.46-2.36.95a7.3 7.3 0 0 0-2.25-1.3L16.2 2.6h-4l-.32 2.54a7.3 7.3 0 0 0-2.26 1.3L7.26 5.5l-2 3.46 2 1.55a7.3 7.3 0 0 0 0 2.6l-2 1.55 2 3.46 2.36-.95a7.3 7.3 0 0 0 2.25 1.3l.33 2.55h4l.3-2.54a7.3 7.3 0 0 0 2.26-1.3l2.37.95 2-3.46-2-1.55c.07-.42.12-.86.12-1.31z"
              />
            </svg>
            <span class="nav-label">设置</span>
          </button>
        </div>
      </aside>

      <main class="content card">
        <div v-if="errorMsg" class="error">{{ errorMsg }}</div>

        <template v-if="store.view === 'settings'">
          <div class="settings-view">
            <div class="settings-sec">
              <div class="settings-sec-title">扫描目录</div>
              <div v-if="store.scanDirs.length === 0" class="settings-empty">尚未添加音乐文件夹</div>
              <div v-for="dir in store.scanDirs" :key="dir" class="settings-dir">
                <span class="settings-dir-path" :title="dir">{{ dir }}</span>
                <button class="settings-remove" title="移除该目录（含曲库）" @click="removeDir(dir)">
                  ✕
                </button>
              </div>
              <div class="settings-actions">
                <button class="btn primary" @click="addDir">添加文件夹</button>
                <button class="btn" @click="scanAll" :disabled="store.scanning || store.scanDirs.length === 0">
                  {{ store.scanning ? "扫描中…" : "重新扫描全部" }}
                </button>
              </div>
            </div>
            <div class="settings-sec">
              <div class="settings-sec-title">快捷键</div>
              <div class="sk-row"><span class="sk-key">空格</span><span>播放 / 暂停</span></div>
              <div class="sk-row"><span class="sk-key">Ctrl+← →</span><span>上 / 下一首</span></div>
            </div>
            <div class="settings-sec">
              <div class="settings-sec-title">支持格式</div>
              <div class="settings-hint">mp3 / flac / wav / ogg / m4a / mp4 / aac / aiff / mp2 等主流音频格式</div>
            </div>
            <div class="settings-sec">
              <div class="settings-sec-title">操作提示</div>
              <div class="sk-row"><span class="sk-key">双击</span><span>播放该歌曲</span></div>
              <div class="sk-row"><span class="sk-key">右键</span><span>播放 / 收藏 / 添加到歌单</span></div>
              <div class="sk-row"><span class="sk-key">拖动</span><span>调整歌曲、歌单顺序</span></div>
            </div>
          </div>
        </template>

        <template v-else>
        <div v-if="store.songs.length === 0" class="empty">
          <div class="empty-icon"><PIcon name="note" tone="amber" :size="34" /></div>
          <p>还没有音乐，先添加一个音乐文件夹</p>
          <button class="btn primary" @click="addDir">添加文件夹</button>
          <p class="hint">支持 mp3 / flac / wav / ogg / m4a / aac / aiff / mp2 等格式</p>
        </div>

        <template v-else-if="store.songs.length > 0">
            <div v-if="store.search.trim() && filtered.length === 0 && (store.view === 'all' || store.view === 'album' || store.view === 'artist' || store.view === 'fav')" class="settings-empty">
              没有找到「{{ store.search.trim() }}」相关的歌曲
            </div>
            <div v-else-if="store.view === 'fav' && store.songs.length > 0 && favCount === 0" class="settings-empty">
              还没有收藏的歌曲：点歌曲右侧的心形收藏
            </div>
            <div v-if="(store.view === 'all' || store.view === 'fav') && (store.view === 'all' || favCount > 0)" class="sort-bar">
              <span class="sort-label">排序</span>
              <button
                v-for="m in SORT_OPTIONS"
                :key="m.value"
                class="sort-chip"
                :class="{ on: store.sortMode === m.value }"
                @click="store.sortMode = m.value"
              >
                {{ m.label }}
              </button>
              <button
                v-if="store.sortMode !== 'custom'"
                class="sort-arrow"
                :class="{ desc: store.sortDir === 'desc' }"
                :title="store.sortDir === 'asc' ? '切换为降序' : '切换为升序'"
                @click="store.sortDir = store.sortDir === 'asc' ? 'desc' : 'asc'"
              >
                <span class="arrow-up">▲</span>
                <span class="arrow-down">▼</span>
              </button>
            </div>
            <SongTable
              v-if="store.view === 'all' || store.view === 'fav'"
              :songs="filtered"
              :drag-order="store.view === 'all' && store.sortMode === 'custom' && !store.search.trim()"
              @contextmenu="openCtx"
              @reorder="onReorder"
            />
          <template v-else-if="store.view === 'album'">
            <SongTable v-if="store.search.trim()" :songs="filtered" @contextmenu="openCtx" />
            <div v-else-if="!selAlbum" class="group-list">
              <div
                v-for="g in albumGroups"
                :key="g.name"
                class="group-item"
                @click="selAlbum = g.name"
                @dblclick="playGroup(g.songs)"
              >
                <span class="group-name">{{ g.name }}</span>
                <span class="group-count">{{ g.songs.length }} 首</span>
              </div>
            </div>
            <template v-else>
              <div class="group-head">
                <button class="btn" @click="selAlbum = ''">← 返回</button>
                <span class="group-title">{{ selAlbum }}</span>
              </div>
              <SongTable :songs="albumGroups.find((g) => g.name === selAlbum)!.songs" @contextmenu="openCtx" />
            </template>
          </template>
          <template v-else-if="store.view === 'artist'">
            <SongTable v-if="store.search.trim()" :songs="filtered" @contextmenu="openCtx" />
            <div v-else-if="!selArtist" class="group-list">
              <div
                v-for="g in artistGroups"
                :key="g.name"
                class="group-item"
                @click="selArtist = g.name"
                @dblclick="playGroup(g.songs)"
              >
                <span class="group-name">{{ g.name }}</span>
                <span class="group-count">{{ g.songs.length }} 首</span>
              </div>
            </div>
            <template v-else>
              <div class="group-head">
                <button class="btn" @click="selArtist = ''">← 返回</button>
                <span class="group-title">{{ selArtist }}</span>
              </div>
              <SongTable :songs="artistGroups.find((g) => g.name === selArtist)!.songs" @contextmenu="openCtx" />
            </template>
          </template>
          <template v-else-if="store.view === 'playlist' && curPlaylist">
            <div class="group-head">
              <template v-if="renamingId === curPlaylist.id">
                <input
                  v-model="renamingName"
                  class="input ren-input"
                  @keydown.enter="renamePlaylist(curPlaylist.id)"
                  @keydown.esc="renamingId = null"
                />
                <button class="btn small" @click="renamePlaylist(curPlaylist.id)">保存</button>
                <button class="btn small" @click="renamingId = null">取消</button>
              </template>
              <template v-else>
                <span class="group-title">{{ curPlaylist.name }}</span>
                <span class="group-count">{{ playlistSongs.length }} 首</span>
                <button class="btn small" title="播放整个歌单" @click="playPlaylist(curPlaylist)">播放</button>
                <button class="btn small" title="批量添加歌曲" @click="plAddOpenDialog">批量添加歌曲</button>
                <button class="btn small" title="重命名" @click="renamingId = curPlaylist.id; renamingName = curPlaylist.name">重命名</button>
              </template>
            </div>
            <div v-if="playlistSongs.length === 0" class="settings-empty">
              歌单还没有歌曲：点「批量添加歌曲」加入，或右键歌曲「加入歌单」
            </div>
            <div v-else-if="store.search.trim() && plShown.length === 0" class="settings-empty">
              歌单里没有找到「{{ store.search.trim() }}」相关的歌曲
            </div>
            <SongTable
              v-else
              :songs="plShown"
              :reorderable="!store.search.trim()"
              @contextmenu="openCtx"
              @reorder="onReorder"
            />
          </template>
        </template>
        </template>
      </main>
    </div>

    <PlayerBar />

    <QueuePanel />

    <Transition name="qmask">
      <div v-if="plAddOpen" class="pl-add-mask" @click="plAddOpen = false">
        <div class="pl-add-panel card" @click.stop>
          <div class="pl-add-head">
            <span class="pl-add-title">添加歌曲到「{{ curPlaylist?.name }}」</span>
            <button class="q-close" title="关闭" @click="plAddOpen = false">✕</button>
          </div>
          <input
            v-model="plAddQ"
            class="input pl-add-search"
            placeholder="搜索要添加的歌曲…"
            @keydown.esc="plAddOpen = false"
          />
          <div class="pl-add-list">
            <div
              v-for="s in plAddCandidates"
              :key="s.path"
              class="pl-add-row"
              :class="{ sel: plAddSel.includes(s.path) }"
              @click="plAddToggle(s.path)"
            >
              <span class="pl-add-check">
                <PIcon v-if="plAddSel.includes(s.path)" name="list" tone="amber" :size="12" />
              </span>
              <div class="pl-add-meta">
                <div class="pl-add-name">{{ s.title }}</div>
                <div class="pl-add-sub">{{ s.artist || s.album }}</div>
              </div>
              <span class="pl-add-dur">{{ fmtTime(s.duration) }}</span>
            </div>
            <div v-if="plAddCandidates.length === 0" class="pl-add-empty">
              没有可添加的歌曲
            </div>
          </div>
          <div class="pl-add-foot">
            <button class="btn small" @click="plAddToggleAll">全选 / 清空</button>
            <button class="btn primary small" :disabled="plAddSel.length === 0" @click="plAddConfirm">
              添加所选（{{ plAddSel.length }}）
            </button>
          </div>
        </div>
      </div>
    </Transition>

    <Transition name="toast">
      <div v-if="store.toast" class="toast">{{ store.toast }}</div>
    </Transition>

    <Transition name="np">
      <NowPlaying v-if="store.nowPlaying" />
    </Transition>

    <Transition name="ctxfade">
      <div v-if="ctxMenu" class="ctx-mask" @click="closeCtx" @contextmenu.prevent="closeCtx">
        <div class="ctx-menu card" :style="{ left: ctxMenu.x + 'px', top: ctxMenu.y + 'px' }" @click.stop>
          <button class="ctx-item" @click="ctxPlay"><PIcon name="play" :size="14" />播放该首</button>
          <button class="ctx-item" @click="ctxPlayNext"><PIcon name="next" :size="14" />下一首播放</button>
          <div class="ctx-sep" />
          <button class="ctx-item" @click="ctxToggleFav">
            <PIcon :name="ctxMenu.song.fav ? 'heart' : 'heart-o'" :size="14" :tone="ctxMenu.song.fav ? 'amber' : 'light'" />
            {{ ctxMenu.song.fav ? "取消收藏" : "收藏" }}
          </button>
          <button class="ctx-item" @click="ctxOpenAddToPl"><PIcon name="list" :size="14" />添加到歌单</button>
          <button class="ctx-item" @click="ctxReveal"><PIcon name="folder" :size="14" />打开所在文件夹</button>
          <button
            v-if="store.view === 'playlist' && store.selPlaylist !== null"
            class="ctx-item danger"
            @click="ctxRemoveFromPlaylist(store.selPlaylist)"
          >
            <PIcon name="heart-o" :size="14" />从歌单移除
          </button>
          <div class="ctx-sep" />
          <button class="ctx-item danger" @click="ctxRemoveFromQueue">从队列移除</button>
        </div>
      </div>
    </Transition>

    <Transition name="qmask">
      <div v-if="plPickOpen" class="pl-add-mask" @click="plPickOpen = false">
        <div class="pl-pick card" @click.stop>
          <div class="pl-add-head">
            <span class="pl-add-title">添加到歌单</span>
            <span class="pl-pick-song" :title="plPickSong?.title">{{ plPickSong?.title }}</span>
            <button class="q-close" title="关闭" @click="plPickOpen = false">✕</button>
          </div>
          <div class="pl-pick-list">
            <button
              v-for="p in store.playlists"
              :key="p.id"
              class="ctx-item"
              @click="ctxAddToPlaylist(p.id)"
            >
              <PIcon name="list" :size="14" />{{ p.name }}
            </button>
            <div v-if="store.playlists.length === 0" class="pl-pick-empty">
              还没有歌单，先去左侧栏创建
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.np-enter-active,
.np-leave-active {
  transition: opacity 0.3s ease, transform 0.3s ease;
}
.np-enter-from {
  opacity: 0;
  transform: scale(0.95);
}
.np-leave-to {
  opacity: 0;
  transform: scale(1.04);
}

/* 右键菜单 */
.ctx-mask {
  position: fixed;
  inset: 0;
  z-index: 150;
}
.ctx-menu {
  position: absolute;
  min-width: 176px;
  padding: 6px 0;
  background: linear-gradient(180deg, #2a2d36 0%, #23262f 100%);
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 10px;
  box-shadow: 0 10px 36px rgba(0, 0, 0, 0.55);
}
.ctx-sep {
  height: 1px;
  background: rgba(255, 255, 255, 0.07);
  margin: 4px 0;
}

/* 歌单 */
.ctxfade-enter-active,
.ctxfade-leave-active { transition: opacity 0.15s; }
.ctxfade-enter-from,
.ctxfade-leave-to { opacity: 0; }
.mini-app {
  height: 100vh;
  display: flex;
  align-items: center;
  background: linear-gradient(180deg, #2e323b 0%, #262a31 100%);
}
.mini-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 10px;
}
.mini-cover {
  width: 52px;
  height: 52px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  background: linear-gradient(135deg, var(--accent-strong), var(--accent));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.3), 0 2px 6px rgba(0, 0, 0, 0.5);
}
.mini-meta {
  flex: 1;
  min-width: 0;
}
.mini-title {
  font-size: 13px;
  font-weight: 600;
  color: #f2d9a8;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mini-artist {
  font-size: 11px;
  color: var(--text-dim);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.mini-controls {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.mini-fav {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.5);
  background: linear-gradient(180deg, #4a4f5a 0%, #34373f 100%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.15);
  color: var(--text-dim);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.85;
}
.mini-fav:hover {
  filter: brightness(1.2);
}
.mini-exit {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 1px solid rgba(0, 0, 0, 0.5);
  background: linear-gradient(180deg, #4a4f5a 0%, #34373f 100%);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.15);
  color: var(--text-dim);
  font-size: 13px;
  line-height: 1;
  cursor: pointer;
}
.mini-exit:hover {
  filter: brightness(1.2);
}
.win-btns {
  display: flex;
  align-items: stretch;
  align-self: stretch;
}
.win-btn {
  width: 42px;
  height: 36px;
  border: none;
  border-left: 1px solid rgba(0, 0, 0, 0.25);
  background: linear-gradient(180deg, #454a54 0%, #33373f 100%);
  color: #cfc7b4;
  font-size: 13px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}
.win-btn:hover {
  background: linear-gradient(180deg, #525763 0%, #3d414b 100%);
  color: #fff;
}
.win-btn.close:hover {
  background: linear-gradient(180deg, #e05252 0%, #c23a3a 100%);
  color: #fff;
}
.app {
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 0 12px 18px;
  background: var(--bg-2);
  border-bottom: 1px solid var(--line);
  flex-shrink: 0;
}
.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-right: auto;
}
.logo {
  width: 36px;
  height: 36px;
  border-radius: var(--radius);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 19px;
  color: #241a04;
  background: linear-gradient(135deg, var(--accent-strong), var(--accent));
  box-shadow: var(--shadow);
}
.brand h1 {
  font-size: 15px;
  font-weight: 600;
  letter-spacing: 0.5px;
}
.toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-left: auto;
}
.queue-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 40px;
  padding: 0 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--card);
  color: var(--text-dim);
  cursor: pointer;
  font-family: inherit;
  font-size: 12.5px;
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
.toast {
  position: fixed;
  left: 50%;
  bottom: 84px;
  transform: translateX(-50%);
  z-index: 200;
  padding: 9px 18px;
  border-radius: 20px;
  background: linear-gradient(180deg, #3a3f4b, #262a33 60%, #20242c);
  border: 1px solid rgba(0, 0, 0, 0.55);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.1), 0 6px 20px rgba(0, 0, 0, 0.5);
  color: #f2d9a8;
  font-size: 12.5px;
  pointer-events: none;
}
.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.25s, transform 0.25s;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(8px);
}
.search-box {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 40px;
  padding: 0 14px;
  border: 1px solid var(--line);
  border-radius: var(--radius);
  background: var(--card);
  transition: border-color 0.15s;
}
.search-box:focus-within {
  border-color: var(--accent);
}
.search-icon {
  color: var(--text-dim);
  font-size: 15px;
}
.search-box input {
  border: none;
  outline: none;
  background: none;
  color: var(--text);
  font-family: inherit;
  font-size: 14px;
  width: 240px;
}
@media (max-width: 1100px) {
  .search-box input {
    width: 170px;
  }
}
@media (max-width: 900px) {
  .search-box input {
    width: 120px;
  }
  .toolbar {
    padding-left: 12px;
    gap: 8px;
  }
  .sidebar {
    width: 154px;
  }
}
.settings-view {
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 26px;
  max-width: 760px;
}
.settings-sec-title {
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.5px;
  color: var(--text-dim);
  margin-bottom: 10px;
}
.settings-empty {
  font-size: 13px;
  color: var(--text-dim);
  padding: 8px 0 12px;
}
.settings-dir {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  background: var(--card-2);
  margin-bottom: 6px;
}
.settings-dir-path {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.settings-remove {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 50%;
  background: rgba(224, 108, 90, 0.15);
  color: #ff9b8a;
  font-size: 11px;
  cursor: pointer;
}
.settings-remove:hover {
  background: rgba(224, 108, 90, 0.3);
}
.settings-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.body {
  flex: 1;
  display: flex;
  gap: 10px;
  padding: 12px 16px;
  min-height: 0;
}

.sidebar {
  width: 176px;
  flex-shrink: 0;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.nav-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 13px;
  border: none;
  border-radius: 8px;
  background: none;
  color: var(--text-dim);
  font-family: inherit;
  font-size: 14px;
  cursor: pointer;
  text-align: left;
  transition: background 0.12s, color 0.12s;
}
.nav-btn:hover {
  background: var(--card-2);
  color: var(--text);
}
.nav-btn.active {
  background: linear-gradient(135deg, var(--accent-soft), rgba(232, 163, 61, 0.08));
  color: var(--accent-strong);
  font-weight: 600;
}
.nav-btn .nav-label {
  flex: 1;
}
.nav-btn .count {
  font-size: 12px;
  color: var(--text-dim);
}
.sidebar-pl,
.pl-section {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 280px;
  overflow-y: auto;
  margin-top: 2px;
  padding: 6px 0;
  border-top: 1px solid var(--line);
}
.pl-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 12px 6px;
}
.pl-title {
  flex: 1;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--text-dim);
}
.pl-row {
  padding: 5px 6px 5px 12px;
  font-size: 13px;
  cursor: grab;
  user-select: none;
}
.pl-row.drag-over {
  background: rgba(232, 163, 61, 0.15);
  outline: 1px solid rgba(232, 163, 61, 0.45);
}
.pl-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pl-new {
  padding: 4px 8px 4px 24px;
}
.pl-new .input {
  height: 28px;
  font-size: 12px;
}
.pl-new-btn {
  padding: 6px 12px 6px 24px;
  font-size: 12.5px;
  color: var(--text-dim);
}
.ren-input {
  flex: 1;
  max-width: 260px;
  height: 28px;
}
.sidebar-bottom {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 10px 4px 2px;
  border-top: 1px solid var(--line);
}
.sk-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 8px;
  font-size: 13px;
  color: var(--text-dim);
}
.sk-key {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 52px;
  padding: 3px 8px;
  border-radius: 6px;
  background: var(--card-2);
  border: 1px solid var(--line);
  font-size: 12px;
  color: var(--text-dim);
  flex-shrink: 0;
  font-family: monospace;
}
.settings-hint {
  font-size: 13px;
  color: var(--text-dim);
  padding: 2px 8px;
}
.settings-btn {
  margin-top: 6px;
}

.content {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: 4px;
  position: relative;
}

/* 排序栏 */
.sort-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  flex-wrap: wrap;
}
.sort-label {
  font-size: 11px;
  color: var(--text-dim);
  margin-right: 4px;
}
.sort-chip {
  padding: 4px 12px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: var(--card-2);
  color: var(--text-dim);
  font-family: inherit;
  font-size: 11.5px;
  cursor: pointer;
  transition: border-color 0.15s, color 0.15s, background 0.15s;
}
.sort-chip:hover {
  border-color: rgba(232, 163, 61, 0.45);
  color: var(--text);
}
.sort-chip.on {
  border-color: rgba(232, 163, 61, 0.6);
  background: linear-gradient(135deg, var(--accent-soft), rgba(232, 163, 61, 0.08));
  color: var(--accent-strong);
  font-weight: 600;
}
/* 升降序切换箭头 */
.sort-arrow {
  display: inline-flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
  width: 30px;
  height: 28px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: var(--card-2);
  cursor: pointer;
  transition: border-color 0.15s;
}
.sort-arrow .arrow-up,
.sort-arrow .arrow-down {
  font-size: 7px;
  line-height: 1;
  color: var(--text-dim);
  opacity: 0.45;
}
.sort-arrow .arrow-up {
  color: var(--accent-strong);
  opacity: 1;
}
.sort-arrow.desc .arrow-up {
  color: var(--text-dim);
  opacity: 0.45;
}
.sort-arrow.desc .arrow-down {
  color: var(--accent-strong);
  opacity: 1;
}
.sort-arrow:hover {
  border-color: rgba(232, 163, 61, 0.45);
}

.empty {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--text-dim);
}
.empty-icon {
  font-size: 38px;
  color: var(--accent);
}
.empty .hint {
  font-size: 11px;
  color: #6b6f78;
}

.error {
  position: sticky;
  top: 4px;
  margin: 4px;
  padding: 8px 12px;
  color: #ffd7cf;
  background: rgba(224, 108, 90, 0.15);
  border: 1px solid rgba(224, 108, 90, 0.35);
  border-radius: 8px;
  z-index: 5;
}

/* 批量添加歌曲 */
.pl-add-mask {
  position: fixed;
  inset: 0;
  z-index: 90;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
}
.pl-add-panel {
  width: 460px;
  max-width: 92vw;
  max-height: 76vh;
  display: flex;
  flex-direction: column;
  background: linear-gradient(180deg, #2a2d36 0%, #22252d 100%);
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 12px;
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.6);
  padding: 16px;
  gap: 12px;
}
.pl-pick {
  width: 320px;
  max-width: 90vw;
  max-height: 70vh;
  display: flex;
  flex-direction: column;
  background: linear-gradient(180deg, #2a2d36 0%, #22252d 100%);
  border: 1px solid rgba(0, 0, 0, 0.55);
  border-radius: 12px;
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.6);
  padding: 14px;
  gap: 10px;
}
.pl-pick-song {
  flex: 1;
  min-width: 0;
  font-size: 11.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: right;
}
.pl-pick-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
  padding: 4px 0;
  max-height: 46vh;
}
.pl-pick-list .ctx-item {
  width: 100%;
  text-align: left;
  font-size: 12.5px;
}
.pl-pick-empty {
  padding: 26px 0;
  text-align: center;
  font-size: 12px;
  color: var(--text-dim);
}
.pl-add-head {
  display: flex;
  align-items: center;
  gap: 10px;
}
.pl-add-title {
  flex: 1;
  font-size: 14px;
  font-weight: 700;
  color: #f5e3bc;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pl-add-panel .q-close {
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
.pl-add-panel .q-close:hover {
  filter: brightness(1.2);
}
.pl-add-search {
  height: 32px;
  font-size: 12.5px;
}
.pl-add-list {
  flex: 1;
  overflow-y: auto;
  min-height: 120px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.18);
  padding: 4px 0;
}
.pl-add-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  cursor: pointer;
  transition: background 0.12s;
}
.pl-add-row:hover {
  background: rgba(255, 255, 255, 0.05);
}
.pl-add-row.sel {
  background: rgba(232, 163, 61, 0.12);
}
.pl-add-check {
  width: 18px;
  height: 18px;
  border-radius: 5px;
  border: 1px solid rgba(255, 255, 255, 0.25);
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.35);
}
.pl-add-row.sel .pl-add-check {
  border-color: var(--accent-strong);
  background: rgba(232, 163, 61, 0.2);
}
.pl-add-meta {
  flex: 1;
  min-width: 0;
}
.pl-add-name {
  font-size: 12.5px;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pl-add-sub {
  font-size: 10.5px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pl-add-dur {
  font-size: 10.5px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}
.pl-add-empty {
  padding: 36px 0;
  text-align: center;
  font-size: 12px;
  color: var(--text-dim);
}
.pl-add-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.qmask-enter-active,
.qmask-leave-active {
  transition: opacity 0.2s ease;
}
.qmask-enter-active .pl-add-panel,
.qmask-leave-active .pl-add-panel {
  transition: transform 0.22s ease;
}
.qmask-enter-from,
.qmask-leave-to {
  opacity: 0;
}
.qmask-enter-from .pl-add-panel,
.qmask-leave-to .pl-add-panel {
  transform: scale(0.96);
}
</style>
