import { computed, ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { store, setPlayMode, refreshPlaylists, showToast } from "./stores/player";
import { setVolume, seekTo, seekPreview } from "./player";
import type { PlayMode, Playlist, Song } from "./types";

export const MODE_ICON: Record<PlayMode, "loop" | "shuffle" | "repeat-one"> = {
  order: "loop",
  shuffle: "shuffle",
  repeat: "repeat-one",
};
export const MODE_LABEL: Record<PlayMode, string> = {
  order: "顺序播放",
  shuffle: "随机播放",
  repeat: "单曲循环",
};
export function cycleMode() {
  const order: PlayMode[] = ["order", "shuffle", "repeat"];
  const i = order.indexOf(store.playMode);
  setPlayMode(order[(i + 1) % order.length]);
}

export const totalDur = computed(() => store.current?.duration || store.duration || 0);

// 进度条：拖动时 seekPreview 只预览，松手后真正 seek
export function useSeek(el: Ref<HTMLElement | null>) {
  const seekPct = computed(() => {
    const total = totalDur.value;
    if (!total) return 0;
    return Math.min(100, (store.position / total) * 100);
  });
  let dragging = false;
  function secOf(e: PointerEvent): number {
    const t = el.value;
    const total = totalDur.value;
    if (!t || !total) return -1;
    const rect = t.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    return Math.max(0, Math.min(total, ratio * total));
  }
  function seekDown(e: PointerEvent) {
    const sec = secOf(e);
    if (sec < 0) return;
    dragging = true;
    store.scrubbing = true;
    seekPreview(sec);
    window.addEventListener("pointermove", seekMove);
    window.addEventListener("pointerup", seekUp);
  }
  function seekMove(e: PointerEvent) {
    if (!dragging) return;
    const sec = secOf(e);
    if (sec >= 0) seekPreview(sec);
  }
  function seekUp(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    store.scrubbing = false;
    window.removeEventListener("pointermove", seekMove);
    window.removeEventListener("pointerup", seekUp);
    const sec = secOf(e);
    if (sec >= 0) seekTo(sec).catch(() => {});
  }
  return { seekPct, seekDown };
}

export const VOL_STEPS = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
export function useVolume(el: Ref<HTMLElement | null>) {
  const volPct = computed(() => Math.round((Math.round(store.volume * 10) / 10) * 100));
  let dragging = false;
  function volSet(e: PointerEvent) {
    const t = el.value;
    if (!t) return;
    const rect = t.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / rect.width;
    const step = Math.max(0, Math.min(10, Math.round(ratio * 10)));
    setVolume(step / 10);
  }
  function volDown(e: PointerEvent) {
    dragging = true;
    volSet(e);
    window.addEventListener("pointermove", volMove);
    window.addEventListener("pointerup", volUp);
  }
  function volMove(e: PointerEvent) {
    if (dragging) volSet(e);
  }
  function volUp() {
    if (!dragging) return;
    dragging = false;
    window.removeEventListener("pointermove", volMove);
    window.removeEventListener("pointerup", volUp);
  }
  return { volPct, volDown };
}

// ---- 收藏：全界面统一 toggle（库内同步） ----
export async function toggleFav(song: Song): Promise<void> {
  const fav = await invoke<number>("toggle_fav", { path: song.path });
  song.fav = fav;
  const s = store.songs.find((x) => x.path === song.path);
  if (s) s.fav = fav;
}

// ---- 加入歌单选择器（PlayerBar / NowPlaying 共用） ----
export function usePlaylistPicker() {
  const open = ref(false);
  const playlists = ref<Playlist[]>([]);
  const menuRef = ref<HTMLElement | null>(null);
  async function toggle() {
    playlists.value = await invoke<Playlist[]>("list_playlists");
    open.value = !open.value;
  }
  async function add(song: Song | null, id: number) {
    if (!song) return;
    await invoke("add_to_playlist", { id, path: song.path });
    open.value = false;
    await refreshPlaylists();
    const p = playlists.value.find((x) => x.id === id);
    showToast(`已加入「${p?.name ?? "歌单"}」`);
  }
  function closeOutside(e: PointerEvent) {
    if (open.value && menuRef.value && !menuRef.value.contains(e.target as Node)) {
      open.value = false;
    }
  }
  return { open, playlists, menuRef, toggle, add, closeOutside };
}
