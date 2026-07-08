import { ref, nextTick } from "vue";

export function useQrCode() {
  const qrDataUrl = ref("");

  async function generate(data: string) {
    qrDataUrl.value = "";
    await nextTick();
    try {
      const QRCode = (await import("qrcode")).default;
      qrDataUrl.value = await QRCode.toDataURL(data, {
        width: 280,
        margin: 2,
        color: { dark: "#000000", light: "#ffffff" },
      });
      return qrDataUrl.value;
    } catch {
      qrDataUrl.value = "";
      return "";
    }
  }

  return { qrDataUrl, generate };
}
