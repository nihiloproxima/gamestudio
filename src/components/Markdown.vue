<script setup lang="ts">
import { computed } from "vue";
import { marked } from "marked";
import DOMPurify from "dompurify";
import { openExternal } from "../lib/native";

/** Markdown from the data files. Sanitized: item bodies can come from agents or pasted bug reports. */
const props = defineProps<{ source: string }>();

const html = computed(() => DOMPurify.sanitize(marked.parse(props.source, { async: false, gfm: true, breaks: true })));

function onClick(e: MouseEvent) {
  const a = (e.target as HTMLElement).closest("a");
  if (!a) return;
  e.preventDefault();
  if (a.href.startsWith("http")) openExternal(a.href);
}
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html -- sanitized above -->
  <div class="prose" @click="onClick" v-html="html" />
</template>
