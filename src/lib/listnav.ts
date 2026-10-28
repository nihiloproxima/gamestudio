import { onBeforeUnmount, onMounted, ref, type Ref } from "vue";
import type { Item } from "../domain/types";
import { store } from "../state/store";
import { isTyping } from "./native";

/** j/k or arrows move through a list of items, Enter opens the selected one. Inactive while a modal is up. */
export function useListNav(items: Ref<Item[]>) {
  const selected = ref<string | null>(null);

  function onKey(e: KeyboardEvent) {
    if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey) return;
    if (store.palette || store.capture || store.launch) return;
    const list = items.value;
    if (!list.length) return;
    const at = list.findIndex((i) => i.id === selected.value);
    if (e.key === "ArrowDown" || e.key === "j" || e.key === "ArrowUp" || e.key === "k") {
      const down = e.key === "ArrowDown" || e.key === "j";
      const next = at < 0 ? (down ? 0 : list.length - 1) : Math.min(list.length - 1, Math.max(0, at + (down ? 1 : -1)));
      selected.value = list[next].id;
      document.querySelector(`[data-item="${selected.value}"]`)?.scrollIntoView({ block: "nearest" });
      if (store.drawer) store.drawer = selected.value;
    } else if (e.key === "Enter" && at >= 0) {
      store.drawer = list[at].id;
    } else {
      return;
    }
    e.preventDefault();
  }

  onMounted(() => window.addEventListener("keydown", onKey));
  onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
  return selected;
}
