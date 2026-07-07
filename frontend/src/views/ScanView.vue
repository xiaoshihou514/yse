<template>
  <div id="scan-view">
    <div class="dark-top" />
    <div class="dark-bottom" />
    <div class="dark-left" />
    <div class="dark-right" />
    <div class="frame-corner tl" />
    <div class="frame-corner tr" />
    <div class="frame-corner bl" />
    <div class="frame-corner br" />
    <div class="scan-line" />
    <p class="hint">{{ hintText }}</p>
    <div class="footer">
      <t-button size="small" @click="cancelScan">取消</t-button>
      <t-button size="small" variant="outline" @click="pickImage"
        >上传图片</t-button
      >
    </div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, onUnmounted } from "vue";
import { useRouter, useRoute } from "vue-router";
import { MessagePlugin } from "tdesign-vue-next";

const router = useRouter();
const route = useRoute();
const scanType = (route.query.type as string) || "config";
const hintText =
  scanType === "contact" ? "扫描对方的名片二维码" : "将二维码置于框内";

let cancel: (() => void) | null = null;

onMounted(async () => {
  await nextTick();
  try {
    const mod = await import("@tauri-apps/plugin-barcode-scanner");
    const perm = await mod.checkPermissions();
    if (perm !== "granted") {
      const result = await mod.requestPermissions();
      if (result !== "granted") {
        await MessagePlugin.warning("摄像头权限被拒绝");
        router.back();
        return;
      }
    }
    cancel = () => {
      void mod.cancel().catch(() => {});
    };
    const result = await mod.scan({
      windowed: true,
      formats: [mod.Format.QRCode],
      cameraDirection: "back",
    });
    if (scanType === "contact") {
      const data = JSON.parse(result.content);
      router.replace({
        name: "contacts",
        query: { scanName: data.name, scanHostname: data.hostname },
      });
    } else {
      router.replace({ name: "config", query: { scanResult: result.content } });
    }
  } catch (e: any) {
    const msg = String(e?.message ?? e);
    if (msg.includes("cancelled")) {
      router.back();
    } else {
      await MessagePlugin.info(`扫码未成功，请选择二维码图片`);
      router.back();
    }
  }
});

onUnmounted(() => {
  cancel?.();
});

function cancelScan() {
  cancel?.();
  router.back();
}

function pickImage() {
  cancel?.();
  const input = document.createElement("input");
  input.type = "file";
  input.accept = "image/*";
  input.onchange = async () => {
    const file = input.files?.[0];
    if (!file) return;
    const { Html5Qrcode } = await import("html5-qrcode");
    const c = new Html5Qrcode("scan-view");
    try {
      const r = await c.scanFile(file, false);
      if (scanType === "contact") {
        const data = JSON.parse(r);
        router.replace({
          name: "contacts",
          query: { scanName: data.name, scanHostname: data.hostname },
        });
      } else {
        router.replace({ name: "config", query: { scanResult: r } });
      }
    } finally {
      try {
        await c.clear();
      } catch {}
    }
  };
  input.click();
}
</script>

<style scoped>
#scan-view {
  position: fixed;
  inset: 0;
  z-index: 100000;
  background: transparent;
}
/* Four dark panels around the scan area */
.dark-top {
  position: fixed;
  left: 0;
  right: 0;
  top: 0;
  height: 25vh;
  background: rgba(0, 0, 0, 0.55);
}
.dark-bottom {
  position: fixed;
  left: 0;
  right: 0;
  top: calc(25vh + 280px);
  bottom: 0;
  background: rgba(0, 0, 0, 0.55);
}
.dark-left {
  position: fixed;
  left: 0;
  top: 25vh;
  bottom: calc(100% - 25vh - 280px);
  width: calc(50% - 140px);
  background: rgba(0, 0, 0, 0.55);
}
.dark-right {
  position: fixed;
  right: 0;
  top: 25vh;
  bottom: calc(100% - 25vh - 280px);
  width: calc(50% - 140px);
  background: rgba(0, 0, 0, 0.55);
}
/* Frame corners */
.frame-corner {
  position: fixed;
  width: 28px;
  height: 28px;
  border-color: var(--td-brand-color);
  border-style: solid;
}
.frame-corner.tl {
  left: calc(50% - 140px - 3px);
  top: calc(25vh - 3px);
  border-width: 3px 0 0 3px;
  border-radius: 4px 0 0 0;
}
.frame-corner.tr {
  right: calc(50% - 140px - 3px);
  top: calc(25vh - 3px);
  border-width: 3px 3px 0 0;
  border-radius: 0 4px 0 0;
}
.frame-corner.bl {
  left: calc(50% - 140px - 3px);
  top: calc(25vh + 280px - 3px);
  border-width: 0 0 3px 3px;
  border-radius: 0 0 0 4px;
}
.frame-corner.br {
  right: calc(50% - 140px - 3px);
  top: calc(25vh + 280px - 3px);
  border-width: 0 3px 3px 0;
  border-radius: 0 0 4px 0;
}
/* Scan line */
.scan-line {
  position: fixed;
  left: calc(50% - 132px);
  right: calc(50% - 132px);
  height: 2px;
  background: var(--td-brand-color);
  border-radius: 1px;
  opacity: 0.8;
  z-index: 1;
  animation: scanMove 2s ease-in-out infinite;
}
@keyframes scanMove {
  0% {
    top: calc(25vh + 16px);
  }
  50% {
    top: calc(25vh + 262px);
  }
  100% {
    top: calc(25vh + 16px);
  }
}
/* Hint text */
.hint {
  position: fixed;
  top: calc(25vh + 290px);
  left: 0;
  right: 0;
  text-align: center;
  color: #fff;
  font-size: 14px;
  z-index: 2;
}
/* Footer */
.footer {
  position: fixed;
  bottom: 40px;
  left: 0;
  right: 0;
  display: flex;
  justify-content: center;
  gap: 12px;
  z-index: 2;
}
</style>
