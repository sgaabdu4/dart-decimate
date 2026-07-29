import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const migrationCheck = resolve("scripts/check-dart-decimate-migration.mjs");

test("migration check skips frozen lifecycle artifacts", () => {
  const fixture = mkdtempSync(join(tmpdir(), "dart-decimate-migration-"));
  write(
    fixture,
    "features/example/PLAN.md",
    `Run the completed local ${"Deci" + "mate"} binary.\n`,
  );
  write(
    fixture,
    "features/example/receipts/S-1.json",
    `{"review":"legacy ${"Deci" + "mate"} wording"}\n`,
  );

  const result = run(fixture);

  assert.equal(result.status, 0, result.stderr);
});

test("migration check still rejects old names in product source", () => {
  const fixture = mkdtempSync(join(tmpdir(), "dart-decimate-migration-"));
  write(
    fixture,
    "README.md",
    `Run the completed local ${"Deci" + "mate"} binary.\n`,
  );

  const result = run(fixture);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /old product name/);
});

function run(cwd) {
  return spawnSync(process.execPath, [migrationCheck], {
    cwd,
    encoding: "utf8",
  });
}

function write(root, relative, contents) {
  const file = join(root, relative);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, contents);
}
