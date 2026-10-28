<script setup lang="ts">
import { computed } from "vue";
import { CircleCheck, CircleX, ExternalLink, GitMerge, GitPullRequest, GitPullRequestClosed, GitPullRequestDraft, LoaderCircle, RefreshCw } from "lucide-vue-next";
import type { PullRequest } from "../domain/types";
import { itemsById, now, project, refreshPrs, store } from "../state/store";
import { mentionedIds } from "../lib/links";
import { since } from "../lib/format";
import { openExternal } from "../lib/native";

const open = computed(() => (store.prs ?? []).filter((p) => p.state === "open" || p.state === "draft"));
const closed = computed(() => (store.prs ?? []).filter((p) => p.state === "merged" || p.state === "closed").slice(0, 20));

function items(pr: PullRequest) {
  const key = project.value?.key ?? "";
  return [...new Set([...mentionedIds(pr.branch, key), ...mentionedIds(pr.title, key)])].filter((id) => itemsById.value.has(id));
}

const REVIEW = { approved: "approuvée", changes: "changements demandés", required: "revue demandée", none: "" };
</script>

<template>
  <div class="view">
    <header class="view-head" data-tauri-drag-region>
      <h1>GitHub</h1>
      <button v-if="project?.github" class="repo code" @click="openExternal(`https://github.com/${project.github}`)">
        {{ project.github }}<ExternalLink :size="12" />
      </button>
      <span class="spacer" />
      <button class="btn btn-ghost" @click="refreshPrs"><RefreshCw />Actualiser</button>
    </header>

    <div class="view-body">
      <p v-if="store.prsError" class="empty">{{ store.prsError }}</p>
      <p v-else-if="store.prs === null" class="empty">Chargement des pull requests…</p>
      <template v-else>
        <template v-for="group in [{ label: 'Ouvertes', list: open }, { label: 'Fermées récemment', list: closed }]" :key="group.label">
          <section v-if="group.list.length" class="group">
            <h2 class="section-title">{{ group.label }}<span class="count">{{ group.list.length }}</span></h2>
            <button v-for="pr in group.list" :key="pr.number" class="pr" @click="openExternal(pr.url)">
              <GitMerge v-if="pr.state === 'merged'" :size="16" class="icon merged" />
              <GitPullRequestClosed v-else-if="pr.state === 'closed'" :size="16" class="icon closed" />
              <GitPullRequestDraft v-else-if="pr.state === 'draft'" :size="16" class="icon draft" />
              <GitPullRequest v-else :size="16" class="icon open" />
              <span class="num">#{{ pr.number }}</span>
              <span class="body">
                <span class="title">{{ pr.title }}</span>
                <span class="sub">
                  <span class="code">{{ pr.branch }}</span> vers <span class="code">{{ pr.base }}</span>
                  <template v-if="pr.author">, par {{ pr.author }}</template>
                </span>
              </span>
              <span v-for="id in items(pr)" :key="id" class="chip item" @click.stop="store.drawer = id">{{ id }}</span>
              <span v-if="REVIEW[pr.review]" class="review" :class="pr.review">{{ REVIEW[pr.review] }}</span>
              <span v-if="pr.checks !== 'none'" class="checks" :class="pr.checks">
                <CircleCheck v-if="pr.checks === 'pass'" :size="14" />
                <CircleX v-else-if="pr.checks === 'fail'" :size="14" />
                <LoaderCircle v-else :size="14" class="spin" />
              </span>
              <span class="diff"><span class="add">+{{ pr.additions }}</span> <span class="del">−{{ pr.deletions }}</span></span>
              <span class="when">{{ since(pr.mergedAt ?? pr.updatedAt, now) }}</span>
            </button>
          </section>
        </template>
        <div v-if="!open.length && !closed.length" class="empty">
          <GitPullRequest />
          <p>Aucune pull request sur {{ project?.github }}.</p>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.repo {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--overlay);
  font-size: var(--text-xs);
}
.repo:hover {
  color: var(--text);
}
.group + .group {
  margin-top: 28px;
}
.pr {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  min-height: 52px;
  padding: 6px 12px;
  border-radius: var(--radius-sm);
  text-align: left;
}
.pr:hover {
  background: rgb(255 255 255 / 0.03);
}
.icon.open {
  color: var(--ok);
}
.icon.draft,
.icon.closed {
  color: var(--overlay);
}
.icon.merged {
  color: var(--review);
}
.num {
  width: 46px;
  color: var(--overlay);
  font-size: var(--text-sm);
}
.body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex: 1;
  min-width: 0;
}
.title,
.sub {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sub {
  color: var(--overlay);
  font-size: var(--text-xs);
}
.item {
  color: var(--accent);
}
.review {
  font-size: var(--text-xs);
  color: var(--subtext);
}
.review.approved {
  color: var(--ok);
}
.review.changes {
  color: var(--pr-high);
}
.checks.pass {
  color: var(--ok);
}
.checks.fail {
  color: var(--fail);
}
.checks.pending {
  color: var(--pending);
}
.diff {
  width: 90px;
  font-size: var(--text-xs);
  text-align: right;
}
.add {
  color: var(--ok);
}
.del {
  color: var(--fail);
}
.when {
  width: 90px;
  color: var(--overlay);
  font-size: var(--text-xs);
  text-align: right;
}
</style>
