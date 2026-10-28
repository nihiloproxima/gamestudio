<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { Terminal, type ITheme } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import "@xterm/xterm/css/xterm.css";
import { api, errorText } from "../lib/api";

const props = defineProps<{ session: string }>();

// Catppuccin Mocha with MesloLGL Nerd Font (bundled as "Studio Mono", see styles/base.css).
const FONT = '"Studio Mono", Menlo, monospace';
const MOCHA: ITheme = {
  background: "#1e1e2e",
  foreground: "#cdd6f4",
  cursor: "#f5e0dc",
  cursorAccent: "#1e1e2e",
  selectionBackground: "#585b70",
  black: "#45475a",
  red: "#f38ba8",
  green: "#a6e3a1",
  yellow: "#f9e2af",
  blue: "#89b4fa",
  magenta: "#f5c2e7",
  cyan: "#94e2d5",
  white: "#bac2de",
  brightBlack: "#585b70",
  brightRed: "#f38ba8",
  brightGreen: "#a6e3a1",
  brightYellow: "#f9e2af",
  brightBlue: "#89b4fa",
  brightMagenta: "#f5c2e7",
  brightCyan: "#94e2d5",
  brightWhite: "#a6adc8",
};

const host = ref<HTMLDivElement>();
const error = ref("");
let term: Terminal | undefined;
let ptyId: number | undefined;
let observer: ResizeObserver | undefined;
let disposed = false;

onMounted(async () => {
  // The glyph atlas is drawn once: both weights must be loaded before the terminal renders anything.
  await Promise.all(['13px "Studio Mono"', 'bold 13px "Studio Mono"'].map((f) => document.fonts.load(f))).catch(
    () => undefined,
  );
  if (disposed || !host.value) return;

  const t = new Terminal({
    fontFamily: FONT,
    fontSize: 13,
    lineHeight: 1,
    theme: MOCHA,
    cursorBlink: true,
    macOptionIsMeta: true,
    macOptionClickForcesSelection: true,
    // tmux owns the history; scroll inside the pane goes to tmux.
    scrollback: 0,
  });
  term = t;
  const fit = new FitAddon();
  t.loadAddon(fit);
  t.open(host.value);
  try {
    t.loadAddon(new WebglAddon());
  } catch {
    // The DOM renderer is slower but always works.
  }
  fit.fit();

  // Claude Code reads Option+Enter (ESC CR) as a newline; make Shift+Enter do the same.
  t.attachCustomKeyEventHandler((e) => {
    if (e.key === "Enter" && e.shiftKey && !e.metaKey) {
      if (e.type === "keydown" && ptyId !== undefined) api.ptyWrite(ptyId, "\x1b\r");
      return false;
    }
    return true;
  });

  try {
    ptyId = await api.ptyAttach(props.session, t.cols, t.rows, (bytes) => t.write(bytes));
  } catch (e) {
    error.value = errorText(e);
    return;
  }
  if (disposed) {
    api.ptyDetach(ptyId);
    return;
  }
  t.onData((data) => ptyId !== undefined && api.ptyWrite(ptyId, data));
  t.onResize(({ cols, rows }) => ptyId !== undefined && api.ptyResize(ptyId, cols, rows));
  observer = new ResizeObserver(() => fit.fit());
  observer.observe(host.value);
  t.focus();
});

onBeforeUnmount(() => {
  disposed = true;
  observer?.disconnect();
  if (ptyId !== undefined) api.ptyDetach(ptyId);
  term?.dispose();
});
</script>

<template>
  <div class="term">
    <div ref="host" class="host" />
    <p v-if="error" class="error" role="alert">{{ error }}</p>
  </div>
</template>

<style scoped>
.term {
  position: relative;
  flex: 1;
  min-height: 0;
  padding: 12px 6px 6px 14px;
  background: #1e1e2e;
}
.host {
  width: 100%;
  height: 100%;
}
.error {
  position: absolute;
  inset: 16px 20px auto;
  color: var(--fail);
  font-size: var(--text-sm);
}
</style>
