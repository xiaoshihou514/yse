import { createApp } from "vue";
import { createPinia } from "pinia";
import "./style.css";
import TDesign from "tdesign-vue-next";
import "tdesign-vue-next/es/style/index.css";
import "highlight.js/styles/github-dark.min.css";

import App from "./App.vue";
import router from "./router";

// Restore dark mode preference
if (localStorage.getItem("yse-dark") === "true") {
  document.documentElement.setAttribute("theme-mode", "dark");
}

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.use(TDesign);
app.mount("#app");
