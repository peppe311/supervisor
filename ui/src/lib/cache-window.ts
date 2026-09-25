import type { NativeUsage } from "./native-usage";

const WINDOW_MS = 30 * 60 * 1000;

// OpenAI documents a 30-minute minimum for GPT-5.6 and later. Earlier
// models and other providers have different retention rules.
function hasDocumentedWindow(model: string | null): boolean {
  return model !== null && /^gpt-(?:[6-9](?:\.\d+)?|5\.(?:[6-9]|[1-9]\d+))(?:-|$)/.test(model);
}

function positiveCount(value: string | null): boolean {
  return value !== null && BigInt(value) > 0n;
}

export interface CacheWindowEstimate {
  label: string;
  elapsed: boolean;
  description: string;
}

export function cacheWindowEstimate(view: NativeUsage | null, nowMs: number): CacheWindowEstimate | null {
  const last = view?.report?.last;
  const observed = view?.cacheReportAtMs;
  if (!view?.visible || !view.connected || !view.current || !last || observed === null || observed === undefined
      || !hasDocumentedWindow(view.model) || !Number.isFinite(nowMs) || observed > nowMs
      || (!positiveCount(last.cachedInputTokens) && !positiveCount(last.cacheWriteInputTokens))) return null;

  // The App Server reports cache token counts, not expiration or the exact
  // model-call timestamp. Count from local receipt and label the result an estimate.
  const remaining = WINDOW_MS - Math.max(0, nowMs - observed);
  const description = "Estimate from the last Codex usage report. OpenAI documents a 30-minute minimum for this model family; Codex does not report the cache expiry. Reuse may refresh the window.";
  if (remaining <= 0) return {label:"30m+",elapsed:true,description};
  const seconds = Math.ceil(remaining / 1000);
  return {label:`${String(Math.floor(seconds / 60)).padStart(2,"0")}:${String(seconds % 60).padStart(2,"0")}`,elapsed:false,description};
}
