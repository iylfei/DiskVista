import type { Window as DesktopWindow } from "@tauri-apps/api/window";

type WindowStateSource = Pick<DesktopWindow, "isMaximized" | "onResized">;

/** Observe native state, including double-clicks and Windows keyboard shortcuts. */
export function observeMaximized(
  source: WindowStateSource,
  onChange: (maximized: boolean) => void,
  onError: (error: unknown) => void,
): () => void {
  let active = true;
  let revision = 0;
  let unlisten: (() => void) | undefined;
  const report = (error: unknown) => {
    if (active) onError(error);
  };
  const refresh = async () => {
    const request = ++revision;
    try {
      const maximized = await source.isMaximized();
      if (active && request === revision) onChange(maximized);
    } catch (error) {
      report(error);
    }
  };
  source
    .onResized(() => void refresh())
    .then((dispose) => {
      // React StrictMode can unmount before the asynchronous listener is ready.
      if (!active) return dispose();
      unlisten = dispose;
      void refresh();
    })
    .catch(report);
  return () => {
    active = false;
    unlisten?.();
  };
}
