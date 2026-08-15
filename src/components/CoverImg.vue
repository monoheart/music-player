<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { coverCache } from "../cover-cache";
import PIcon from "./PIcon.vue";

const props = defineProps<{ path: string; size?: number; lazy?: boolean }>();

const dataUrl = ref("");
const el = ref<HTMLElement | null>(null);
let observed = false;

// 全局共享一个 IntersectionObserver（大列表下避免每行一个观察器）
let sharedIO: IntersectionObserver | null = null;
let ioCount = 0;
const ioJobs = new WeakMap<Element, () => void>();
function observeCover(el: Element, job: () => void) {
  if (!sharedIO) {
    sharedIO = new IntersectionObserver(
      (entries) => {
        for (const en of entries) {
          if (en.isIntersecting) {
            sharedIO?.unobserve(en.target);
            const cb = ioJobs.get(en.target);
            ioJobs.delete(en.target);
            cb?.();
          }
        }
      },
      { rootMargin: "80px" },
    );
  }
  ioJobs.set(el, job);
  ioCount++;
  sharedIO.observe(el);
}
function unobserveCover(el: Element) {
  sharedIO?.unobserve(el);
  if (ioJobs.delete(el)) {
    ioCount--;
    if (ioCount === 0 && sharedIO) {
      sharedIO.disconnect();
      sharedIO = null;
    }
  }
}

async function load() {
  let v = coverCache.get(props.path);
  if (v === undefined) {
    const p = await invoke<string | null>("get_cover", { path: props.path });
    v = p ? convertFileSrc(p) : "";
    coverCache.set(props.path, v);
  }
  dataUrl.value = v;
}

function job() {
  load().catch(() => {});
}

onMounted(() => {
  if (props.lazy && typeof IntersectionObserver !== "undefined" && el.value) {
    observed = true;
    observeCover(el.value, job);
  } else {
    load().catch(() => {});
  }
});

watch(
  () => props.path,
  () => {
    if (observed && el.value) {
      unobserveCover(el.value);
      observeCover(el.value, job);
    } else {
      load().catch(() => {});
    }
  },
);

onBeforeUnmount(() => {
  if (observed && el.value) unobserveCover(el.value);
});
</script>

<template>
  <span ref="el" v-if="dataUrl" class="cover-img" :style="{ width: size + 'px', height: size + 'px' }">
    <img :key="dataUrl" :src="dataUrl" alt="" />
  </span>
  <span v-else ref="el" class="cover-img placeholder" :style="{ width: size + 'px', height: size + 'px' }">
    <PIcon name="note" tone="amber" :size="Math.round((size ?? 16) * 0.8)" />
  </span>
</template>

<style scoped>
.cover-img {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  overflow: hidden;
  border-radius: 8px;
  background: linear-gradient(135deg, var(--accent-strong), var(--accent));
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.25), 0 1px 3px rgba(0, 0, 0, 0.4);
  border: 1px solid rgba(0, 0, 0, 0.35);
}
.cover-img img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  animation: cover-in 0.35s ease;
}
@keyframes cover-in {
  from {
    opacity: 0;
    transform: scale(0.92);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}
</style>