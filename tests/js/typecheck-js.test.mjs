import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { runNode } from "./child-process.mjs";

const typecheck = resolve("scripts/typecheck-js.mjs");

/** @param {Record<string, string>} files */
function tree(files) {
  const root = mkdtempSync(join(tmpdir(), "dart-decimate-typecheck-"));
  for (const directory of ["npm/bin", "scripts", "tests/js", "tests/npm"]) {
    mkdirSync(join(root, directory), { recursive: true });
  }
  for (const [path, source] of Object.entries(files)) {
    writeFileSync(join(root, path), source);
  }
  return root;
}

test("JS syntax check passes valid scripts in every checked root", async () => {
  const root = tree({
    "npm/bin/cli.js": "module.exports = 1;\n",
    "scripts/guard.mjs": "export const ok = true;\n",
    "tests/js/case.test.mjs": "export {};\n",
    "tests/npm/smoke.js": "console.log('ok');\n",
    "scripts/notes.txt": "not JavaScript {\n",
  });

  const result = await runNode([typecheck], { cwd: root });

  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "Checked 4 JavaScript files.");
});

test("JS syntax check fails on a script that does not parse", async () => {
  const root = tree({ "tests/npm/broken.js": "const = ;\n" });

  const result = await runNode([typecheck], { cwd: root });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /broken\.js/);
  assert.match(result.stderr, /SyntaxError/);
});
