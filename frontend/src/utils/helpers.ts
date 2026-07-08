import { MessagePlugin } from "tdesign-vue-next";

export function showError(label: string, e: unknown): void {
  MessagePlugin.error(`${label}失败: ${e}`).catch(() => {});
}
