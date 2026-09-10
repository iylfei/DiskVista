import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const desktop = new URL("../../../apps/desktop/", import.meta.url);
const read = (path: string) => readFileSync(new URL(path, desktop), "utf8");

describe("desktop command permissions", () => {
  it("allows every registered application command from the main window", () => {
    const handler = read("src/main.rs").match(
      /tauri::generate_handler!\[([\s\S]*?)\]/,
    );
    expect(handler).not.toBeNull();
    const commands = [...handler![1].matchAll(/\w+::(\w+)/g)].map(
      (match) => match[1],
    );
    expect(commands.length).toBeGreaterThan(0);
    const manifest = read("build.rs");
    const capability = JSON.parse(read("capabilities/main.json"));
    expect(capability.windows).toContain("main");
    for (const command of commands) {
      expect(manifest, `Missing build manifest command: ${command}`).toContain(
        `"${command}"`,
      );
      expect(
        capability.permissions,
        `Missing permission: ${command}`,
      ).toContain(`allow-${command.replaceAll("_", "-")}`);
    }
  });
});
