import { createRouter, createWebHistory } from "vue-router";
import ChatView from "@/views/ChatView.vue";
import PluginView from "@/views/PluginView.vue";
import ConfigView from "@/views/ConfigView.vue";
import LogView from "@/views/LogView.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "chat", component: ChatView },
    { path: "/plugins", name: "plugins", component: PluginView },
    { path: "/config", name: "config", component: ConfigView },
    { path: "/logs", name: "logs", component: LogView },
  ],
});

export default router;
