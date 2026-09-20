export type GitSection = "staged" | "unstaged" | "untracked";
export interface GitEntry { path: string; iconKey: string; sections: GitSection[]; blocked: string | null }
export const gitSections: {key: GitSection; label: string}[] = [
  {key: "staged", label: "Staged"}, {key: "unstaged", label: "Unstaged"}, {key: "untracked", label: "Untracked"},
];
export function groupedGitEntries(entries: GitEntry[], query = ""): {key: GitSection; label: string; entries: GitEntry[]}[] {
  const needle = query.toLocaleLowerCase();
  return gitSections.map(section => ({...section, entries: entries.filter(entry => entry.sections.includes(section.key) && entry.path.toLocaleLowerCase().includes(needle))}));
}
export function matchesProjectDiffReply(detail: unknown, owner: string, viewId: string, requestId: string, open: boolean): boolean {
  if (!open || !owner || !viewId || !requestId || !detail || typeof detail !== "object") return false;
  const reply = detail as Record<string, unknown>;
  return reply.owner === owner && reply.viewId === viewId && reply.requestId === requestId;
}
