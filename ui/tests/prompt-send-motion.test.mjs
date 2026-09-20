import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import {
  animatePromptArrival,
  animateSendControl,
  armPromptSendMotion,
  clearPromptSendMotion,
  consumePromptSendMotion,
  transferPromptArrival,
} from "../src/lib/prompt-send-motion.ts";

test("prompt motion is one-shot and scoped to the exact conversation and text", () => {
  clearPromptSendMotion("main");
  clearPromptSendMotion("graph:other");
  armPromptSendMotion("main", "  Ship this change  ", 1_000);
  armPromptSendMotion("graph:other", "Graph request", 1_000);

  assert.equal(consumePromptSendMotion("main", { role:"assistant", text:"Ship this change" }, 1_001), false);
  assert.equal(consumePromptSendMotion("main", { role:"user", text:"Another prompt" }, 1_001), false);
  assert.equal(consumePromptSendMotion("graph:other", { role:"user", text:"Ship this change" }, 1_001), false);
  assert.equal(consumePromptSendMotion("main", { role:"user", kind:"message", text:"Ship this change" }, 1_002), true);
  assert.equal(consumePromptSendMotion("main", { role:"user", kind:"message", text:"Ship this change" }, 1_003), false);
  assert.equal(consumePromptSendMotion("graph:other", { role:"user", text:"Graph request" }, 1_003), true);
});

test("expired or future prompt intents cannot animate unrelated history", () => {
  clearPromptSendMotion("main");
  armPromptSendMotion("main", "Old prompt", 2_000);
  assert.equal(consumePromptSendMotion("main", { role:"user", text:"Old prompt" }, 17_001), false);
  armPromptSendMotion("main", "Future prompt", 20_000);
  assert.equal(consumePromptSendMotion("main", { role:"user", text:"Future prompt" }, 19_999), false);
  assert.equal(consumePromptSendMotion("main", { role:"user", text:"Future prompt" }, 20_001), false);
});

test("the prompt bubble uses the deliberate visible launch recipe", () => {
  const previous = {
    window: globalThis.window,
    document: globalThis.document,
    getComputedStyle: globalThis.getComputedStyle,
  };
  let captured;
  const animation = new EventTarget();
  animation.cancel = () => animation.dispatchEvent(new Event("cancel"));
  const row = {
    dataset: {},
    animate(keyframes, options) { captured = { keyframes, options }; return animation; },
  };
  globalThis.window = { matchMedia: () => ({ matches:false }) };
  globalThis.document = { documentElement:{} };
  globalThis.getComputedStyle = () => ({
    getPropertyValue(name) {
      if (name === "--ca-prompt-send-duration") return "1200ms";
      if (name === "--ca-shadow-float") return "0 18px 48px transparent";
      return "";
    },
  });
  try {
    animatePromptArrival(row);
    assert.equal(captured.options.duration,1200);
    assert.equal(captured.keyframes.length,5);
    assert.match(captured.keyframes[0].transform,/24px/);
    assert.equal(captured.keyframes[1].offset,.3);
    assert.equal(captured.keyframes[2].offset,.68);
    assert.equal(captured.keyframes[3].offset,.86);
    assert.equal(row.dataset.promptMotion,"entering");
    animation.dispatchEvent(new Event("finish"));
    assert.equal(row.dataset.promptMotion,undefined);
  } finally {
    if (previous.window === undefined) delete globalThis.window; else globalThis.window = previous.window;
    if (previous.document === undefined) delete globalThis.document; else globalThis.document = previous.document;
    if (previous.getComputedStyle === undefined) delete globalThis.getComputedStyle; else globalThis.getComputedStyle = previous.getComputedStyle;
  }
});

test("the send control uses the same complete 1200 ms interval", () => {
  const previous = {
    window: globalThis.window,
    document: globalThis.document,
    getComputedStyle: globalThis.getComputedStyle,
  };
  let captured;
  const animation = new EventTarget();
  animation.cancel = () => animation.dispatchEvent(new Event("cancel"));
  const control = {
    dataset: {},
    querySelectorAll: () => [],
    animate(keyframes, options) { captured = { keyframes, options }; return animation; },
  };
  globalThis.window = { matchMedia: () => ({ matches:false }) };
  globalThis.document = { documentElement:{} };
  globalThis.getComputedStyle = () => ({
    getPropertyValue: name => name === "--ca-prompt-send-duration" ? "1200ms" : "",
  });
  try {
    animateSendControl(control);
    assert.equal(captured.options.duration,1200);
    assert.equal(control.dataset.sendMotion,"launching");
    animation.dispatchEvent(new Event("finish"));
    assert.equal(control.dataset.sendMotion,undefined);
  } finally {
    if (previous.window === undefined) delete globalThis.window; else globalThis.window = previous.window;
    if (previous.document === undefined) delete globalThis.document; else globalThis.document = previous.document;
    if (previous.getComputedStyle === undefined) delete globalThis.getComputedStyle; else globalThis.getComputedStyle = previous.getComputedStyle;
  }
});

test("prompt motion keeps its elapsed time when native reconciliation replaces the row", () => {
  const previous = {
    window: globalThis.window,
    document: globalThis.document,
    getComputedStyle: globalThis.getComputedStyle,
  };
  const animations = [];
  const row = () => ({
    dataset: {},
    animate() {
      const animation = new EventTarget();
      animation.currentTime = 0;
      animation.cancel = () => animation.dispatchEvent(new Event("cancel"));
      animations.push(animation);
      return animation;
    },
  });
  globalThis.window = { matchMedia: () => ({ matches:false }) };
  globalThis.document = { documentElement:{} };
  globalThis.getComputedStyle = () => ({ getPropertyValue: () => "" });
  const initial = row();
  const accepted = row();
  try {
    animatePromptArrival(initial);
    animations[0].currentTime = 417;
    assert.equal(transferPromptArrival(initial, accepted), true);
    assert.equal(animations.length,2);
    assert.equal(animations[1].currentTime,417);
    assert.equal(initial.dataset.promptMotion,undefined);
    assert.equal(accepted.dataset.promptMotion,"entering");
  } finally {
    if (previous.window === undefined) delete globalThis.window; else globalThis.window = previous.window;
    if (previous.document === undefined) delete globalThis.document; else globalThis.document = previous.document;
    if (previous.getComputedStyle === undefined) delete globalThis.getComputedStyle; else globalThis.getComputedStyle = previous.getComputedStyle;
  }
});

test("main and graph arm the shared timeline motion with reduced-motion support", () => {
  const timeline = fs.readFileSync(new URL("../src/lib/agent-timeline.ts", import.meta.url), "utf8");
  const motion = fs.readFileSync(new URL("../src/lib/prompt-send-motion.ts", import.meta.url), "utf8");
  const main = fs.readFileSync(new URL("../src/components/Composer.svelte", import.meta.url), "utf8");
  const graph = fs.readFileSync(new URL("../src/components/AgentGraphCard.svelte", import.meta.url), "utf8");
  const graphTimeline = fs.readFileSync(new URL("../src/components/GraphTimeline.svelte", import.meta.url), "utf8");
  const host = fs.readFileSync(new URL("../../assets/agent-panel.html", import.meta.url), "utf8");

  assert.match(main, /armPromptSendMotion\(conversationKey, message\)/);
  assert.match(graph, /armPromptSendMotion\(owner,message\)/);
  assert.match(graphTimeline, /promptMotionScope:owner/);
  assert.match(host, /promptMotionScope: \(\) => chatMessages\.dataset\.promptMotionScope/);
  assert.match(timeline, /promptsNeedingMotion\.forEach\(animatePromptArrival\)/);
  assert.match(timeline, /transferPromptArrival\(previous, replacement\)/);
  assert.match(motion, /prefers-reduced-motion: reduce/);
  assert.match(motion, /--ca-prompt-send-duration/);
  assert.match(motion, /--ca-ease-emphasized/);
});
