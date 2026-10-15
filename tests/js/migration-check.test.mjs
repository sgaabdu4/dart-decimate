import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { runNode } from "./child-process.mjs";

const migrationCheck = resolve("scripts/check-dart-decimate-migration.mjs");

test("migration check skips frozen lifecycle artifacts", async () => {
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

  const result = await run(fixture);

  assert.equal(result.status, 0, result.stderr);
});

test("migration check skips generated coverage reports", async () => {
  const fixture = mkdtempSync(join(tmpdir(), "dart-decimate-migration-"));
  write(
    fixture,
    "coverage/osv.json",
    `{"path":"/tmp/${"deci" + "mate"}-checkout/pnpm-lock.yaml"}\n`,
  );

  const result = await run(fixture);

  assert.equal(result.status, 0, result.stderr);
});

test("migration check still rejects old names in product source", async () => {
  const fixture = mkdtempSync(join(tmpdir(), "dart-decimate-migration-"));
  write(
    fixture,
    "README.md",
    `Run the completed local ${"Deci" + "mate"} binary.\n`,
  );

  const result = await run(fixture);

  assert.equal(result.status, 1);
  assert.match(result.stderr, /old product name/);
});

/** @param {string} cwd */
function run(cwd) {
  return runNode([migrationCheck], { cwd });
}

/** @param {string} root @param {string} relative @param {string} contents */
function write(root, relative, contents) {
  const file = join(root, relative);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, contents);
}
