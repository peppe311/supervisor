export type LiveDiff = {
  additions: number;
  deletions: number;
  fileCount: number;
  active: boolean;
};

function count(value: unknown): number | null {
  return typeof value === "number"
    && Number.isSafeInteger(value)
    && value >= 0
    ? value
    : null;
}

export function liveDiff(value: unknown): LiveDiff | null {
  if (!value || typeof value !== "object") return null;
  const source = value as Record<string, unknown>;
  const additions = count(source.additions);
  const deletions = count(source.deletions);
  const fileCount = count(source.fileCount);
  if (additions === null || deletions === null || fileCount === null) return null;
  const active = source.active === true;
  if (!active && additions === 0 && deletions === 0 && fileCount === 0) return null;
  return { additions, deletions, fileCount, active };
}

export function liveDiffLabel(value: LiveDiff): string {
  const scope = value.active ? "Live code changes" : "Last turn code changes";
  const files = `${value.fileCount} ${value.fileCount === 1 ? "file" : "files"}`;
  const additions = `${value.additions} ${value.additions === 1 ? "line" : "lines"} added`;
  const deletions = `${value.deletions} ${value.deletions === 1 ? "line" : "lines"} removed`;
  return `${scope}: ${additions}, ${deletions} across ${files}`;
}
