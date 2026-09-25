export const usageCounters = [
  ["totalTokens", "Total"], ["inputTokens", "Input"],
  ["cachedInputTokens", "Cached input"], ["cacheWriteInputTokens", "Cache write input"],
  ["outputTokens", "Output"], ["reasoningOutputTokens", "Reasoning output"],
] as const;
type Counter = typeof usageCounters[number][0];
export type Breakdown = Record<Counter, string | null>;
export interface NativeUsage {
  visible: boolean; connected: boolean; current: boolean;
  turnId: string | null; activeTurnId: string | null;
  model: string | null; cacheReportAtMs: number | null;
  report: {last: Breakdown; total: Breakdown; modelContextWindow: string | null} | null;
}
const record = (value:unknown):Record<string,unknown> => value && typeof value === "object" ? value as Record<string,unknown> : {};
const count = (value:unknown):string|null => typeof value === "string" && /^(0|[1-9][0-9]*)$/.test(value) ? value : null;
const breakdown = (value:unknown):Breakdown => Object.fromEntries(usageCounters.map(([key])=>[key,count(record(value)[key])])) as Breakdown;
export function usageSnapshot(detail:unknown, owner:string):NativeUsage|null {
  const data=record(detail);
  if(!owner || data.owner !== owner || !("nativeUsage" in data)) return null;
  const view=record(data.nativeUsage), report=record(view.report);
  return {visible:view.visible === true,connected:view.connected === true,current:view.current === true,
    turnId:typeof view.turnId === "string" ? view.turnId : null,activeTurnId:typeof view.activeTurnId === "string" ? view.activeTurnId : null,
    model:typeof view.model === "string" ? view.model : null,
    cacheReportAtMs:typeof view.cacheReportAtMs === "number" && Number.isSafeInteger(view.cacheReportAtMs) && view.cacheReportAtMs>0 ? view.cacheReportAtMs : null,
    report:view.report && typeof view.report === "object" ? {last:breakdown(report.last),total:breakdown(report.total),modelContextWindow:count(report.modelContextWindow)} : null};
}
export function formatUsage(value:string|null|undefined):string {
  return value == null ? "Not reported" : BigInt(value).toLocaleString("en-US");
}
export function usagePresentation(view:NativeUsage|null) {
  const report=view?.report, used=report?.last.totalTokens ?? null, capacity=report?.modelContextWindow ?? null;
  const previous=Boolean(view?.activeTurnId && view.turnId !== view.activeTurnId);
  const stale=Boolean(report && (!view?.current || previous));
  // Only the latest native report is compared with capacity, never lifetime usage.
  const basis=used !== null && capacity !== null && BigInt(capacity)>0n ? (BigInt(used)*10000n)/BigInt(capacity) : null;
  const percent=basis === null ? null : `${basis/100n}${basis%100n ? `.${String(basis%100n).padStart(2,"0")}` : ""}%`;
  const meter=basis === null ? null : Number(basis>10000n ? 10000n : basis)/100;
  const summary=!report ? "Waiting for usage" : `${formatUsage(used)}${capacity !== null ? ` / ${formatUsage(capacity)}` : " tokens"}${percent === null ? "" : ` · ${percent}`}`;
  const status=!view?.connected ? "Disconnected · last known report, not live data."
    : !report ? "Waiting for the first native token-usage event."
    : !view.current ? "Previous connection's report · waiting for a fresh native update."
    : previous ? "Previous turn's report · waiting for current turn usage."
    : "Latest native report; not a per-token live count.";
  return {summary,status,stale,meter,percent};
}
