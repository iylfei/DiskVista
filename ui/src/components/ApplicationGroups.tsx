import { useEffect, useState } from "react";
import { api, bytes } from "../lib/api";
import type { Group } from "../lib/types";

const categoryName = (value: string) =>
  ({
    temporary: "临时数据",
    diagnostic: "诊断记录",
    cache: "可再生成缓存",
    application_data: "应用数据",
    application_container: "应用文件夹",
    application: "应用程序",
    system: "Windows 系统",
    protected: "受保护内容",
    unknown: "用途未知",
  })[value] ?? value;

export default function ApplicationGroups({
  scanId,
  revision,
  owner,
  category,
  onOwner,
  onCategory,
  onError,
}: {
  scanId: string;
  revision: number;
  owner: string | null;
  category: string | null;
  onOwner: (value: string | null) => void;
  onCategory: (value: string | null) => void;
  onError: (e: unknown) => void;
}) {
  const [owners, setOwners] = useState<Group[]>([]);
  const [categories, setCategories] = useState<Group[]>([]);
  useEffect(() => {
    let live = true;
    Promise.all([
      api<Group[]>("groups", { scanId, kind: "owner" }),
      api<Group[]>("groups", { scanId, kind: "category" }),
    ])
      .then(([o, c]) => {
        if (live) {
          setOwners(o);
          setCategories(c);
        }
      })
      .catch(onError);
    return () => {
      live = false;
    };
  }, [scanId, revision, onError]);
  return (
    <section aria-label="应用与用途聚合">
      <div className="application-groups">
        <button
          className={!owner ? "active" : ""}
          onClick={() => onOwner(null)}
        >
          全部来源
        </button>
        {owners.map((g) => (
          <button
            className={owner === g.name ? "active" : ""}
            key={g.name}
            onClick={() => onOwner(g.name)}
          >
            {g.name}
            <small>{bytes(g.bytes)}</small>
          </button>
        ))}
      </div>
      <div className="application-groups">
        <button
          className={!category ? "active" : ""}
          onClick={() => onCategory(null)}
        >
          全部用途
        </button>
        {categories.map((g) => (
          <button
            className={category === g.name ? "active" : ""}
            key={g.name}
            onClick={() => onCategory(g.name)}
          >
            {categoryName(g.name)}
            <small>{bytes(g.bytes)}</small>
          </button>
        ))}
      </div>
      <p className="footnote">
        按文件聚合，硬链接占用去重；未知实际占用以逻辑大小估算。显示占用最大的前
        100 个来源，各分类大小均为全扫描范围。
      </p>
    </section>
  );
}
