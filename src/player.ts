import { Channel, invoke } from "@tauri-apps/api/core";
import { store, next, prev, setPlayMode, showToast } from "./stores/player";
import type { PlayMode } from "./types";

let ctx: AudioContext | null = null;
let node: AudioWorkletNode | null = null;
let gain: GainNode | null = null;

let ticker = 0;
let baseTime = 0;
let playToken = 0;
let lastSave = 0;
let origSize: { w: number; h: number } | null = null;
let pausePending = false;

export async function toggleMini() {
  const { getCurrentWindow, LogicalSize } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  if (!store.mini) {
    store.queueOpen = false;
    const s = await win.innerSize();
    origSize = { w: s.width, h: s.height };
    await win.setMinSize(new LogicalSize(380, 64));
    await win.setResizable(false);
    await win.setSize(new LogicalSize(400, 76));
    await win.setAlwaysOnTop(true);
    store.mini = true;
  } else {
    await win.setResizable(true);
    await win.setSize(new LogicalSize(Math.max(origSize?.w ?? 1200, 1100), Math.max(origSize?.h ?? 800, 720)));
    await win.setMinSize(new LogicalSize(1100, 720));
    await win.setAlwaysOnTop(false);
    store.mini = false;
  }
}

function saveProgress(force = false) {
  const now = Date.now();
  if (!force && now - lastSave < 5000) return;
  lastSave = now;
  const cur = store.current;
  if (!cur) return;
  invoke("save_progress", {
    songPath: cur.path,
    position: store.position,
    queue: store.queue.map((q) => q.path),
    volume: store.volume,
    playMode: store.playMode,
    sortMode: store.sortMode,
    sortDir: store.sortDir,
  }).catch(() => {});
}

async function ensureCtx() {
  if (ctx) return;
  // 跟随系统输出设备的首选采样率（椒盐 SPW 同款策略）：
  // 文件速率与系统一致时零转换位精确；不一致时由 worklet 一次高质量重采样到系统速率，
  // 系统混音器无需再 SRC，避免双重重采样
  ctx = new AudioContext();
  invoke("log_info", { message: `context rate=${ctx.sampleRate}` }).catch(() => {});
  await ctx.audioWorklet.addModule(new URL("./player-processor.js", import.meta.url));
  node = new AudioWorkletNode(ctx, "player-processor", {
    channelCount: 2,
    channelCountMode: "explicit",
  });
  gain = ctx.createGain();
  gain.gain.value = store.volume;
  node.connect(gain);
  gain.connect(ctx.destination);
  node.port.onmessage = (e) => {
    if (!e.data) return;
    if (e.data.type === "finished") {
      invoke("log_info", { message: `[js] finished ${store.current?.title ?? "?"} mode=${store.playMode}` }).catch(() => {});
      if (store.playing && store.current) {
        if (store.playMode === "repeat") {
          // 单曲循环：流的解码已结束，seek 无效，直接重开流从头播
          play(store.current.path).catch(() => {});
        } else {
          const finished = store.current;
          if (finished) {
            next();
            if (store.current) play(store.current.path).catch(() => {});
          }
        }
      }
    } else if (e.data.type === "ack") {
      invoke("ack_chunks", { n: e.data.n }).catch(() => {});
    } else if (e.data.type === "paused") {
      // worklet 淡出完成：此刻才真正挂起上下文（声音已平滑降到零）
      pausePending = false;
      ctx?.suspend().catch(() => {});
      invoke("set_paused", { paused: true }).catch(() => {});
    }
  };
  ticker = window.setInterval(() => {
    if (!ctx) return;
    if (store.scrubbing) return;
    const running = ctx.state === "running";
    if (running) {
      const p = Math.max(0, ctx.currentTime - baseTime);
      store.position = p;
      updateLyricIndex();
      saveProgress();
    }
  }, 250);
}

let lyricFetchToken = 0;

function updateLyricIndex() {
  const l = store.lyrics;
  if (l.length === 0) {
    store.lyricIndex = -1;
    return;
  }
  let idx = -1;
  for (let i = 0; i < l.length; i++) {
    if (store.position >= l[i].time) idx = i;
    else break;
  }
  store.lyricIndex = idx;
}

export async function loadLyrics(path: string) {
  const tok = ++lyricFetchToken;
  try {
    const lyrics = await invoke<{ time: number; text: string }[]>("get_lyrics", { path });
    if (tok !== lyricFetchToken) return;
    store.lyrics = lyrics;
    store.lyricIndex = -1;
    updateLyricIndex();
  } catch {
    if (tok === lyricFetchToken) store.lyrics = [];
  }
}

export async function play(path: string) {
  await ensureCtx();
  if (!ctx || !node) return;
  if (ctx.state === "suspended") await ctx.resume();
  clearStream();
  baseTime = ctx.currentTime;
  store.position = 0;
  store.duration = 0;
  store.playing = true;
  const tok = ++playToken;
  const ch = new Channel<any>();
  ch.onmessage = (m) => {
    if (tok !== playToken) return;
    if (m.type === "error") {
      store.playing = false;
      showToast(typeof m.message === "string" && m.message ? m.message : "无法播放该文件");
      return;
    }
    if (m.type === "header") {
      if (m.duration > 0) store.duration = m.duration;
    }
    // 世代（epoch）由后端在每条消息上盖章：seek/切歌后新世代自动作废旧数据
    if (node) node.port.postMessage(m);
  };
  try {
    await invoke("play_stream", { path, channel: ch });
  } catch {
    store.playing = false;
    showToast("无法播放该文件");
    return;
  }
  saveProgress(true);
  loadLyrics(path).catch(() => {});
}

export async function togglePlay() {
  if (!ctx) {
    // 重启后尚未创建音频上下文：若有恢复的当前歌，直接开播并回到上次位置
    if (store.current) {
      await play(store.current.path);
      if (store.position > 0) await seekTo(store.position);
    }
    return;
  }
  if (ctx.state === "running") {
    if (pausePending) {
      // 淡出进行中再点：取消暂停，平滑淡回
      pausePending = false;
      store.playing = true;
      node?.port.postMessage({ type: "resume" });
    } else {
      pausePending = true;
      store.playing = false;
      // worklet 淡出完成后回发 "paused"，届时才真正 suspend（见 onmessage）
      node?.port.postMessage({ type: "pause" });
    }
  } else {
    await ctx.resume();
    store.playing = true;
    invoke("set_paused", { paused: false }).catch(() => {});
    node?.port.postMessage({ type: "resume" });
  }
}

export async function nextTrack() {
  next();
  if (store.current) await play(store.current.path);
}

export async function prevTrack() {
  prev();
  if (store.current) await play(store.current.path);
}

export async function seekTo(sec: number) {
  if (!ctx) return;
  invoke("log_info", { message: `[js] seekTo ${sec.toFixed(2)}` }).catch(() => {});
  baseTime = ctx.currentTime - sec;
  store.position = sec;
  node?.port.postMessage({ type: "clear" });
  await invoke("seek_stream", { seconds: sec }).catch(() => {});
}

export function seekPreview(sec: number) {
  store.position = sec;
}

export function setVolume(v: number) {
  store.volume = v;
  if (gain) gain.gain.value = v;
}

export function clearStream() {
  node?.port.postMessage({ type: "clear" });
}
