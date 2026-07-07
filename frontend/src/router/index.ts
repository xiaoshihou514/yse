import { createRouter, createWebHistory } from "vue-router";
import ChatView from "@/views/ChatView.vue";
import PluginView from "@/views/PluginView.vue";
import ContactsView from "@/views/ContactsView.vue";
import ConfigView from "@/views/ConfigView.vue";
import WelcomeView from "@/views/WelcomeView.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "chat", component: ChatView },
    { path: "/plugins", name: "plugins", component: PluginView },
    { path: "/contacts", name: "contacts", component: ContactsView },
    { path: "/config", name: "config", component: ConfigView },
    { path: "/welcome", name: "welcome", component: WelcomeView },
  ],
});

let _configChecked = false;
let _hasConfig = false;

export function setConfigState(has: boolean) {
  _hasConfig = has;
  _configChecked = true;
}

router.beforeEach((to) => {
  if (to.name === "welcome") return true;
  if (!_configChecked) return true; // still loading
  if (!_hasConfig) return { name: "welcome" };
  return true;
});

export default router;
