<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";

const props = defineProps<{ text: string; center?: boolean }>();
const box = ref<HTMLElement | null>(null);
const inner = ref<HTMLElement | null>(null);
const moving = ref(false);
let checkTimer: ReturnType<typeof setTimeout> | null = null;
// 越长的标题滚动越慢（一秒约滚动 4~6 个字符宽度）
const duration = computed(() => Math.max(12, Math.min(40, Math.round(props.text.length * 0.7))));

function check() {
  if (box.value && inner.value) {
    moving.value = inner.value.scrollWidth > box.value.clientWidth + 4;
  }
}

onMounted(() => {
  check();
  checkTimer = setTimeout(check, 300);
  window.addEventListener("resize", check);
});
onUnmounted(() => {
  window.removeEventListener("resize", check);
  if (checkTimer) clearTimeout(checkTimer);
});
watch(
  () => props.text,
  async () => {
    await nextTick();
    check();
  },
);
</script>

<template>
  <div ref="box" class="mq-box" :class="{ center: props.center }">
    <div ref="inner" class="mq-inner" :class="{ moving }" :style="moving ? { animationDuration: duration + 's' } : undefined">
      <span class="mq-text">{{ text }}</span>
      <span v-if="moving" class="mq-text">{{ text }}</span>
    </div>
  </div>
</template>

<style scoped>
.mq-box {
  overflow: hidden;
  white-space: nowrap;
  min-width: 0;
}
.mq-box.center {
  display: flex;
  justify-content: center;
}
.mq-box.center .mq-inner.moving {
  justify-content: flex-start;
}
.mq-inner {
  display: inline-flex;
  will-change: transform;
}
.mq-text {
  flex-shrink: 0;
  max-width: none;
}
.mq-inner.moving .mq-text {
  padding-right: 40px;
}
.mq-inner.moving {
  animation-name: mq-scroll;
  animation-timing-function: linear;
  animation-iteration-count: infinite;
}
.mq-inner.moving:hover {
  animation-play-state: paused;
}
@keyframes mq-scroll {
  0% {
    transform: translateX(0);
  }
  40% {
    transform: translateX(calc(-50% - 20px));
  }
  60% {
    transform: translateX(calc(-50% - 20px));
  }
  100% {
    transform: translateX(0);
  }
}
</style>
