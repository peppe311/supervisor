import { mount, unmount } from "svelte";
import NativeMedia from "../components/NativeMedia.svelte";

const mounted = new Map<HTMLElement, { update: (value: unknown) => void }>();
let cleanup: MutationObserver | undefined;
export function renderNativeMedia(target: HTMLElement, value: unknown): void {
  const previous = mounted.get(target);
  if (previous) { previous.update(value); return; }
  target.dataset.centralAgentSvelte = "native-media";
  const component = mount(NativeMedia, {target});
  component.update(value);
  mounted.set(target, component);
  if (!cleanup) {
    cleanup = new MutationObserver(() => {
      for (const [host, view] of mounted) {
        if (!host.isConnected) { mounted.delete(host); void unmount(view); }
      }
    });
    cleanup.observe(document.body, {childList: true, subtree: true});
  }
}
