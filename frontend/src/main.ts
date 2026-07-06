import { createApp } from "vue";
import { createPinia } from "pinia";
import "./style.css";
import TDesign from "tdesign-vue-next";
import "tdesign-vue-next/es/style/index.css";
import "highlight.js/styles/github-dark.min.css";

import App from "./App.vue";
import router from "./router";

// Restore theme preference (default to auto)
const theme = localStorage.getItem("yse-theme") || "auto";
if (theme === "dark") {
  document.documentElement.setAttribute("theme-mode", "dark");
} else if (theme === "auto" && window.matchMedia("(prefers-color-scheme: dark)").matches) {
  document.documentElement.setAttribute("theme-mode", "dark");
}

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.use(TDesign);
app.mount("#app");
