// Opt-in measurement of public sky methods. Importing this module performs no
// desktop operation. It neither modifies sky nor intercepts its private transport.
export const marker = "SUPERVISOR_CU_TIMING ";
const setup = new Set(["list_apps", "list_windows", "get_window", "launch_app", "activate_window"]);
const input = new Set(["click", "press_key", "type_text", "scroll", "drag", "set_value", "perform_secondary_action"]);
export function createComputerUseTimer(sky, write, now = () => performance.now()) {
  let sequence = 0;
  return async function measure(method, args) {
    const phase = method === "get_window_state" ? "observation" : setup.has(method) ? "setup" : input.has(method) ? "input" : null;
    if (!phase || typeof sky[method] !== "function") throw new TypeError("Unsupported public Computer Use method");
    if (sequence >= 256) throw new Error("Computer Use probe sample limit reached");
    const sample = { sequence: ++sequence, method, phase };
    if (phase === "observation") {
      sample.screenshotRequested = args?.include_screenshot ?? true;
      sample.textRequested = args?.include_text ?? false;
    }
    const start = now();
    try {
      const result = await sky[method](args);
      sample.completed = true;
      if (phase === "observation") {
        sample.imageReturned = Array.isArray(result?.screenshots) && result.screenshots.length > 0;
        sample.textReturned = !!(result?.accessibility?.tree || result?.accessibility?.document_text);
      }
      return result;
    } catch (error) {
      sample.completed = false;
      throw error; // Preserve outcome uncertainty; never retry.
    } finally {
      sample.durationMs = Math.max(0, now() - start);
      // Only fixed names, booleans and timing values leave the helper. SDK
      // screenshots are already emitted; result and argument payloads stay untouched.
      write("\n" + marker + JSON.stringify(sample) + "\n");
    }
  };
}
