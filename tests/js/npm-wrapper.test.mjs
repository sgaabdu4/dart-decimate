import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

test("npm wrapper exposes the Dart Decimate CLI", () => {
  const result = spawnSync(
    process.execPath,
    ["npm/bin/dart-decimate.js", "--help"],
    {
      cwd: process.cwd(),
      encoding: "utf8",
      env: { ...process.env, DART_DECIMATE_SKIP_BUILD: "1" },
    },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Usage: dart-decimate/);
});
