type PromptMessage = {
  role?: unknown;
  kind?: unknown;
  text?: unknown;
};

type PromptIntent = {
  text: string;
  armedAt: number;
};

const PROMPT_INTENT_TTL_MS = 15_000;
const promptIntents = new Map<string, PromptIntent>();
const promptAnimations = new WeakMap<HTMLElement, Animation>();
const controlAnimations = new WeakMap<HTMLElement, Animation[]>();

function normalizedText(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

export function armPromptSendMotion(scope: string, text: unknown, armedAt = Date.now()): void {
  if (!scope) return;
  promptIntents.set(scope, { text: normalizedText(text), armedAt });
}

export function clearPromptSendMotion(scope: string): void {
  promptIntents.delete(scope);
}

export function consumePromptSendMotion(
  scope: string,
  message: PromptMessage,
  now = Date.now(),
): boolean {
  const intent = promptIntents.get(scope);
  if (!intent) return false;
  if (now < intent.armedAt || now - intent.armedAt > PROMPT_INTENT_TTL_MS) {
    promptIntents.delete(scope);
    return false;
  }
  if (
    message.role !== "user"
    || (message.kind ?? "message") !== "message"
    || normalizedText(message.text) !== intent.text
  ) return false;
  promptIntents.delete(scope);
  return true;
}

function reducedMotion(): boolean {
  return typeof window !== "undefined"
    && typeof window.matchMedia === "function"
    && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function token(name: string, fallback: string): string {
  if (typeof document === "undefined" || typeof getComputedStyle !== "function") return fallback;
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

function duration(name: string, fallback: number): number {
  const value = token(name, `${fallback}ms`);
  const match = value.match(/^([\d.]+)(ms|s)$/);
  if (!match) return fallback;
  const amount = Number(match[1]);
  return Number.isFinite(amount) ? amount * (match[2] === "s" ? 1_000 : 1) : fallback;
}

function finish(
  element: HTMLElement,
  animation: Animation,
  store: WeakMap<HTMLElement, Animation>,
  dataKey: "promptMotion",
): void {
  const clean = () => {
    if (store.get(element) !== animation) return;
    store.delete(element);
    delete element.dataset[dataKey];
  };
  animation.addEventListener("finish", clean, { once: true });
  animation.addEventListener("cancel", clean, { once: true });
}

export function animatePromptArrival(row: HTMLElement): void {
  if (reducedMotion() || typeof row.animate !== "function") return;
  promptAnimations.get(row)?.cancel();
  row.dataset.promptMotion = "entering";
  const emphasized = token("--ca-ease-emphasized", "cubic-bezier(.2, .8, .2, 1)");
  const standard = token("--ca-ease-standard", "cubic-bezier(.2, 0, 0, 1)");
  const liftedShadow = token("--ca-shadow-float", "none");
  const animation = row.animate([
    {
      opacity: .08,
      transform: "translate3d(4px, 24px, 0) scale(.95)",
      filter: "blur(6px)",
      boxShadow: liftedShadow,
      transformOrigin: "bottom right",
      easing: emphasized,
      offset: 0,
    },
    {
      opacity: .72,
      transform: "translate3d(2px, 11px, 0) scale(.98)",
      filter: "blur(2px)",
      boxShadow: liftedShadow,
      transformOrigin: "bottom right",
      easing: emphasized,
      offset: .3,
    },
    {
      opacity: 1,
      transform: "translate3d(0, -3px, 0) scale(1.012)",
      filter: "blur(0)",
      boxShadow: liftedShadow,
      transformOrigin: "bottom right",
      easing: standard,
      offset: .68,
    },
    {
      opacity: 1,
      transform: "translate3d(0, 1px, 0) scale(.997)",
      filter: "blur(0)",
      boxShadow: "none",
      transformOrigin: "bottom right",
      easing: standard,
      offset: .86,
    },
    {
      opacity: 1,
      transform: "translate3d(0, 0, 0) scale(1)",
      filter: "blur(0)",
      boxShadow: "none",
      transformOrigin: "bottom right",
      offset: 1,
    },
  ], { duration: duration("--ca-prompt-send-duration", 1200), easing: "linear" });
  promptAnimations.set(row, animation);
  finish(row, animation, promptAnimations, "promptMotion");
}

export function transferPromptArrival(previousRow: HTMLElement, nextRow: HTMLElement): boolean {
  const previous = promptAnimations.get(previousRow);
  if (!previous || previousRow === nextRow) return previousRow === nextRow && Boolean(previous);
  const elapsed = typeof previous.currentTime === "number" && Number.isFinite(previous.currentTime)
    ? Math.max(0, previous.currentTime)
    : 0;
  previous.cancel();
  animatePromptArrival(nextRow);
  const next = promptAnimations.get(nextRow);
  if (!next) return false;
  next.currentTime = elapsed;
  return true;
}

export function animateSendControl(control: HTMLElement | null | undefined): void {
  if (!control || reducedMotion() || typeof control.animate !== "function") return;
  for (const animation of controlAnimations.get(control) ?? []) animation.cancel();
  control.dataset.sendMotion = "launching";
  const emphasized = token("--ca-ease-emphasized", "cubic-bezier(.2, .8, .2, 1)");
  const standard = token("--ca-ease-standard", "cubic-bezier(.2, 0, 0, 1)");
  const options: KeyframeAnimationOptions = {
    duration: duration("--ca-prompt-send-duration", 1200),
    easing: "linear",
  };
  const button = control.animate([
    { transform: "scale(1)", offset: 0, easing: emphasized },
    { transform: "scale(.84)", offset: .24, easing: emphasized },
    { transform: "scale(1.06)", offset: .62, easing: standard },
    { transform: "scale(.985)", offset: .82, easing: standard },
    { transform: "scale(1)", offset: 1 },
  ], options);
  const animations = [button];
  for (const icon of control.querySelectorAll<HTMLElement>("svg")) {
    if (typeof icon.animate !== "function") continue;
    animations.push(icon.animate([
      { transform: "translate3d(0, 0, 0)", opacity: 1, offset: 0, easing: emphasized },
      { transform: "translate3d(0, -6px, 0)", opacity: .28, offset: .42, easing: standard },
      { transform: "translate3d(0, 2px, 0)", opacity: 1, offset: .72, easing: standard },
      { transform: "translate3d(0, 0, 0)", opacity: 1, offset: 1 },
    ], options));
  }
  controlAnimations.set(control, animations);
  const clean = () => {
    if (controlAnimations.get(control) !== animations) return;
    controlAnimations.delete(control);
    delete control.dataset.sendMotion;
  };
  button.addEventListener("finish", clean, { once: true });
  button.addEventListener("cancel", clean, { once: true });
}
