import { parseAddress } from "@/utils/address";

export function avatarKey(addr: string) {
  const p = parseAddress(addr);
  return `yse-avatar-${p.name}@${p.hostname}`;
}

export function loadAvatar(addr: string): string | null {
  return localStorage.getItem(avatarKey(addr));
}

export function saveAvatar(addr: string, dataUrl: string) {
  localStorage.setItem(avatarKey(addr), dataUrl);
}

export async function pickAvatar(addr: string): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.onchange = () => {
      const f = input.files?.[0];
      if (!f) return resolve(null);
      const reader = new FileReader();
      reader.onload = () => {
        const dataUrl = reader.result as string;
        saveAvatar(addr, dataUrl);
        resolve(dataUrl);
      };
      reader.readAsDataURL(f);
    };
    input.click();
  });
}
