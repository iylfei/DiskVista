import { useRef, useState } from "react";
import type { FileRecord } from "./types";
import { addSelection } from "./selection";

function emptySelection() {
  return {
    pending: new Map<number, FileRecord>(),
    basket: new Map<number, FileRecord>(),
    message: "",
  };
}

export function useCleanupSelection(onError: (error: unknown) => void) {
  const [state, setState] = useState(emptySelection);
  const current = useRef(state);

  function update(next: typeof state) {
    current.current = next;
    setState(next);
  }

  function selectMany(files: FileRecord[]) {
    const previous = current.current;
    try {
      const pending = addSelection(
        previous.pending,
        files.filter((file) => !previous.basket.has(file.id)),
      );
      addSelection(previous.basket, [...pending.values()]);
      update({ ...previous, pending, message: "" });
    } catch (error) {
      onError(error);
    }
  }

  function select(file: FileRecord) {
    const previous = current.current;
    if (file.assessment.risk === "protected" || previous.basket.has(file.id))
      return;
    if (!previous.pending.has(file.id)) {
      selectMany([file]);
      return;
    }
    const pending = new Map(previous.pending);
    pending.delete(file.id);
    update({ ...previous, pending, message: "" });
  }

  function addToBasket() {
    const previous = current.current;
    if (!previous.pending.size) return;
    try {
      const basket = addSelection(previous.basket, [
        ...previous.pending.values(),
      ]);
      const added = basket.size - previous.basket.size;
      update({
        basket,
        pending: new Map(),
        message: `已添加 ${added} 项到待清理清单`,
      });
    } catch (error) {
      onError(error);
    }
  }

  function removeFromBasket(file: FileRecord) {
    const previous = current.current;
    const basket = new Map(previous.basket);
    basket.delete(file.id);
    update({ ...previous, basket, message: "" });
  }

  return {
    basket: state.basket,
    pending: state.pending,
    message: state.message,
    select,
    selectMany,
    addToBasket,
    removeFromBasket,
    clearBasket: () =>
      update({ ...current.current, basket: new Map(), message: "" }),
    reset: () => update(emptySelection()),
  };
}
