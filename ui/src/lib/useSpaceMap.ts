import { useEffect, useState } from "react";
import {
  createSpaceMapCache,
  spaceMapRequestKey,
  type SpaceMapRequest,
  type SpaceMapSnapshot,
} from "./spaceMapData";

const cache = createSpaceMapCache();

export function useSpaceMap(request: SpaceMapRequest) {
  const key = spaceMapRequestKey(request);
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<{
    key: string;
    data: SpaceMapSnapshot | null;
    error: string;
  }>(() => ({ key, data: cache.peek(request), error: "" }));

  useEffect(() => {
    let live = true;
    setState({ key, data: cache.peek(request), error: "" });
    void cache.load(request).then(
      (data) => {
        if (live) setState({ key, data, error: "" });
      },
      () => {
        if (live)
          setState({
            key,
            data: null,
            error: "无法读取这个目录的空间分布，请重试。",
          });
      },
    );
    return () => {
      live = false;
    };
  }, [key, attempt]);

  return {
    data: state.key === key ? state.data : cache.peek(request),
    error: state.key === key ? state.error : "",
    retry: () => {
      cache.invalidate(request);
      setAttempt((value) => value + 1);
    },
  };
}
