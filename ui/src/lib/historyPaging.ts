import { api } from "./api";
import type { HistoryItem, HistoryPage } from "./types";

export const HISTORY_PAGE_SIZE = 20;

export interface HistoryPagingState {
  page: number;
  total: number | null;
  items: HistoryItem[];
  status: "loading" | "ready" | "error";
  error: string;
}

export const initialHistoryState: HistoryPagingState = {
  page: 0,
  total: null,
  items: [],
  status: "loading",
  error: "",
};

export function createHistoryPager({
  onState,
  onError,
}: {
  onState: (state: HistoryPagingState) => void;
  onError: (error: unknown) => void;
}) {
  let live = true;
  let request = 0;
  let state = initialHistoryState;

  function update(next: HistoryPagingState) {
    state = next;
    if (live) onState(next);
  }

  async function load(page = 0): Promise<void> {
    if (!live) return;
    const current = ++request;
    page = Math.max(0, Math.trunc(page));
    update({ ...state, page, items: [], status: "loading", error: "" });
    try {
      const result = await api<HistoryPage>("history_page", {
        offset: page * HISTORY_PAGE_SIZE,
        limit: HISTORY_PAGE_SIZE,
      });
      if (!live || current !== request) return;
      const lastPage = Math.max(
        0,
        Math.ceil(result.total / HISTORY_PAGE_SIZE) - 1,
      );
      if (page > lastPage) {
        await load(lastPage);
        return;
      }
      update({
        page,
        total: result.total,
        items: result.items,
        status: "ready",
        error: "",
      });
    } catch (error) {
      if (!live || current !== request) return;
      update({
        ...state,
        items: [],
        status: "error",
        error: error instanceof Error ? error.message : String(error),
      });
      onError(error);
    }
  }

  void load();
  return {
    load,
    retry: () => load(state.page),
    dispose() {
      live = false;
      request += 1;
    },
  };
}
