import { test } from "node:test";
import assert from "node:assert/strict";
import { convert } from "./vendor-rules.mjs";
const fixture =
  "[Example Cache *]\nDetectFile=%LocalAppData%\\Example\nFileKey1=%LocalAppData%\\Example\\Cache|*|REMOVESELF\n";
test("supports precise directory targets", () =>
  assert.equal(convert(fixture).rules.length, 1));
test("unknown exclusions reject whole entry", () => {
  const r = convert(
    fixture + "ExcludeKey1=FILE|%LocalAppData%\\Example\\Cache|keep\n",
  );
  assert.equal(r.rules.length, 0);
  assert.equal(r.skipped.length, 1);
});
test("a single unsupported target rejects all", () =>
  assert.equal(
    convert(fixture + "FileKey2=%LocalAppData%\\Example|*.db|RECURSE\n").rules
      .length,
    0,
  ));
test("registry and scripts cannot enter pack", () => {
  for (const key of ["RegKey1", "Script", "DetectOS", "Detect"])
    assert.equal(convert(fixture + key + "=anything\n").rules.length, 0);
});
