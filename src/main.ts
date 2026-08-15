import { createApp } from "vue";
import App from "./App.vue";
import { invoke } from "@tauri-apps/api/core";
import "./style.css";

window.onerror = (msg, src, line, col) => {
  invoke("log_error", { message: `onerror: ${msg} @ ${src}:${line}:${col}` }).catch(() => {});
};
window.addEventListener("unhandledrejection", (e) => {
  invoke("log_error", { message: `unhandledrejection: ${String(e.reason)}` }).catch(() => {});
});

createApp(App).mount("#app");