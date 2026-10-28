import { openUrl } from "@tauri-apps/plugin-opener";
import { inTauri } from "./api";

export function openExternal(url: string) {
  if (inTauri) return openUrl(url);
  window.open(url, "_blank", "noopener");
}

export function copy(text: string) {
  return navigator.clipboard.writeText(text);
}

/** Typing in a field: single-key shortcuts must stay out of the way. */
export function isTyping(e: KeyboardEvent) {
  const el = e.target as HTMLElement | null;
  return !!el && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName) || !!el.closest(".xterm"));
}
