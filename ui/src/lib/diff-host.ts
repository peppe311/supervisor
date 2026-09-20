import { mount, unmount } from "svelte";
import DiffViewer from "../components/DiffViewer.svelte";

const mounted = new Map<HTMLElement, { update: (content: string, identity: string) => void }>();
let cleanup: MutationObserver | undefined;

export function renderDiff(target: HTMLElement, content: string, options: { inline?: boolean; identity?: string } = {}): void {
  const previous = mounted.get(target);
  if (previous) { previous.update(content, options.identity ?? ""); return; }
  target.dataset.centralAgentSvelte = "diff-viewer";
  target.replaceChildren();
  mounted.set(target, mount(DiffViewer, { target, props: { content, inline: options.inline ?? false, identity: options.identity ?? "" } }));
  if (!cleanup) {
    cleanup = new MutationObserver(() => {
      for (const [host, component] of mounted) {
        if (!host.isConnected) { mounted.delete(host); void unmount(component); }
      }
    });
    cleanup.observe(document.body, { childList: true, subtree: true });
  }
}
