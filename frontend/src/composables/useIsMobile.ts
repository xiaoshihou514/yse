import { ref } from "vue";

const isMobile = ref(window.innerWidth < 768);

function onResize() {
  isMobile.value = window.innerWidth < 768;
}
window.addEventListener("resize", onResize);

export function useIsMobile() {
  return isMobile;
}
