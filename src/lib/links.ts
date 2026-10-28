import type { PullRequest, Session, Worktree } from "../domain/types";

/** Item ids mentioned in a branch name or a PR title: `jeu-12-armures` and `JEU-12 Armures` both give `JEU-12`. */
export function mentionedIds(text: string | null | undefined, key: string): string[] {
  if (!text || !key) return [];
  const re = new RegExp(`(?:^|[^a-z0-9])${key}-(\\d+)(?![0-9])`, "gi");
  return [...text.matchAll(re)].map((m) => `${key}-${m[1]}`);
}

export const prMentions = (pr: PullRequest, id: string, key: string) =>
  mentionedIds(pr.branch, key).includes(id) || mentionedIds(pr.title, key).includes(id);

export const worktreeMentions = (wt: Worktree, id: string, key: string) => mentionedIds(wt.branch, key).includes(id);

export const sessionMentions = (s: Session, id: string, key: string) =>
  s.item === id || mentionedIds(s.branch, key).includes(id);

/** The PR of a branch, the open one first. */
export function prForBranch(prs: PullRequest[] | null, branch: string | null): PullRequest | undefined {
  if (!prs || !branch) return undefined;
  const mine = prs.filter((p) => p.branch === branch);
  return mine.find((p) => p.state === "open" || p.state === "draft") ?? mine[0];
}
