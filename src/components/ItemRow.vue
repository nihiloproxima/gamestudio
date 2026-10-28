<script setup lang="ts">
import { computed } from "vue";
import { Flag, GitPullRequest, GitMerge } from "lucide-vue-next";
import FieldPick from "./FieldPick.vue";
import KindIcon from "./KindIcon.vue";
import StatusDot from "./StatusDot.vue";
import type { Item } from "../domain/types";
import { milestonesById, now, project, statusOf, store } from "../state/store";
import { prMentions, sessionMentions } from "../lib/links";
import { ago } from "../lib/format";

/** One item on one line: everything changeable in place, a click opens the full record. */
const props = defineProps<{ item: Item; showMilestone?: boolean; selected?: boolean }>();

const key = computed(() => project.value?.key ?? "");
const sessions = computed(() =>
  store.sessions.filter((s) => s.status !== "offline" && sessionMentions(s, props.item.id, key.value)),
);
const pr = computed(() => store.prs?.find((p) => p.state !== "closed" && prMentions(p, props.item.id, key.value)));
const milestone = computed(() => (props.item.milestone ? milestonesById.value.get(props.item.milestone) : undefined));
const closed = computed(() => props.item.status === "done" || props.item.status === "dropped");
</script>

<template>
  <div
    class="row"
    :class="{ selected, closed }"
    role="button"
    tabindex="-1"
    :data-item="item.id"
    @click="store.drawer = item.id"
  >
    <FieldPick :item="item" field="priority" compact />
    <span class="id code">{{ item.id }}</span>
    <FieldPick :item="item" field="status" compact />
    <KindIcon v-if="item.kind !== 'task'" :kind="item.kind" :size="13" />
    <span class="title">{{ item.title }}</span>
    <span class="meta">
      <span v-for="l in item.labels" :key="l" class="chip">{{ l }}</span>
      <span v-if="showMilestone && milestone" class="chip milestone"><Flag />{{ milestone.title }}</span>
      <span v-if="sessions.length" class="live" :title="`${sessions.length} session(s) en cours`">
        <StatusDot :status="statusOf(sessions)" />
      </span>
      <span v-if="pr" class="pr" :class="[pr.state, pr.checks]" :title="`PR #${pr.number}`">
        <GitMerge v-if="pr.state === 'merged'" :size="13" />
        <GitPullRequest v-else :size="13" />
        <span>#{{ pr.number }}</span>
      </span>
      <span class="when">{{ ago(item.updated, now) }}</span>
    </span>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 38px;
  padding: 0 12px 0 6px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 90ms;
}
.row:hover {
  background: rgb(255 255 255 / 0.03);
}
.row.selected {
  background: var(--accent-soft);
  box-shadow: inset 2px 0 0 var(--accent);
}
.id {
  flex: none;
  width: 58px;
  color: var(--overlay);
  font-size: var(--text-xs);
}
.title {
  flex: 1;
  min-width: 0;
  margin-left: 4px;
  overflow: hidden;
  color: var(--text);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.closed .title {
  color: var(--overlay);
}
.meta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: none;
}
.milestone {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.milestone svg {
  color: var(--accent);
}
.live {
  display: grid;
  place-items: center;
  width: 14px;
}
.pr {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--subtext);
  font-size: var(--text-xs);
  font-weight: 600;
}
.pr.draft {
  color: var(--overlay);
}
.pr.merged {
  color: var(--review);
}
.pr.fail {
  color: var(--fail);
}
.pr.pass.open {
  color: var(--ok);
}
.when {
  width: 64px;
  color: var(--overlay);
  font-size: var(--text-xs);
  text-align: right;
}
</style>
