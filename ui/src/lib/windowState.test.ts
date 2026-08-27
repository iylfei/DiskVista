import { describe, it, expect, vi } from "vitest";
import { observeMaximized } from "./windowState";

const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

function fixture() {
  let resize = () => {};
  const unlisten = vi.fn();
  const source = {
    isMaximized: vi.fn(async () => false),
    onResized: vi.fn(async (handler: () => void) => {
      resize = handler;
      return unlisten;
    }),
  };
  return { source, unlisten, resize: () => resize() };
}

describe("native window state", () => {
  it("reads initial state and follows native resize events", async () => {
    const f = fixture();
    const update = vi.fn();
    const fail = vi.fn();
    const stop = observeMaximized(f.source, update, fail);
    await flush();
    expect(update).toHaveBeenLastCalledWith(false);
    f.source.isMaximized.mockResolvedValue(true);
    f.resize();
    await flush();
    expect(update).toHaveBeenLastCalledWith(true);
    stop();
    expect(f.unlisten).toHaveBeenCalledOnce();
    expect(fail).not.toHaveBeenCalled();
  });

  it("cleans up a listener that resolves after StrictMode unmount", async () => {
    const f = fixture();
    const update = vi.fn();
    observeMaximized(f.source, update, vi.fn())();
    await flush();
    expect(f.unlisten).toHaveBeenCalledOnce();
    expect(update).not.toHaveBeenCalled();
  });

  it("ignores stale reads and results after unmount", async () => {
    const f = fixture();
    let stale!: (value: boolean) => void;
    f.source.isMaximized.mockImplementationOnce(
      () => new Promise<boolean>((resolve) => (stale = resolve)),
    );
    const update = vi.fn();
    const stop = observeMaximized(f.source, update, vi.fn());
    await flush();
    f.source.isMaximized.mockResolvedValue(true);
    f.resize();
    await flush();
    expect(update).toHaveBeenCalledExactlyOnceWith(true);
    stale(false);
    await flush();
    expect(update).toHaveBeenCalledTimes(1);
    f.resize();
    stop();
    await flush();
    expect(update).toHaveBeenCalledTimes(1);
  });

  it("reports permission failures without an unhandled rejection", async () => {
    const f = fixture();
    const fail = vi.fn();
    f.source.onResized.mockRejectedValue(new Error("denied"));
    const stop = observeMaximized(f.source, vi.fn(), fail);
    await flush();
    expect(fail).toHaveBeenCalledWith(new Error("denied"));
    stop();
  });
});
