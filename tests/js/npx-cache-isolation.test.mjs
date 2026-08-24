import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, delimiter, dirname, join } from "node:path";
import { test } from "node:test";

const cases = [
  {
    name: "CLI local npx test",
    script: "npm/scripts/test-npx-local.js",
    tempPrefix: "dart-decimate-npx-",
  },
  {
    name: "MCP local npx test",
    script: "npm/scripts/test-npx-mcp-local.js",
    tempPrefix: "dart-decimate-npx-mcp-",
  },
];

for (const fixture of cases) {
  test(`${fixture.name} uses and removes a disposable npm cache`, () => {
    const testDir = mkdtempSync(join(tmpdir(), "dart-decimate-cache-test-"));
    const binDir = join(testDir, "bin");
    const recordPath = join(testDir, "cache-path.txt");
    const inheritedCache = join(testDir, "inherited-cache");
    mkdirSync(binDir);

    const fakeNpxSource = `
const { writeFileSync } = require("node:fs");
writeFileSync(process.env.DART_DECIMATE_CACHE_RECORD, process.env.npm_config_cache || "");
if (process.argv.includes("dart-decimate-mcp")) {
  process.stdout.write('{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25"}}\\n');
} else {
  process.stdout.write("Usage: dart-decimate\\n");
}
`;
    const fakeNpxScript = join(binDir, "fake-npx.cjs");
    const fakeNpx = join(
      binDir,
      process.platform === "win32" ? "npx.cmd" : "npx",
    );
    writeFileSync(fakeNpxScript, fakeNpxSource);
    if (process.platform === "win32") {
      writeFileSync(
        fakeNpx,
        `@echo off\r\n"${process.execPath}" "%~dp0fake-npx.cjs" %*\r\n`,
      );
    } else {
      writeFileSync(fakeNpx, `#!/usr/bin/env node\n${fakeNpxSource}`);
      chmodSync(fakeNpx, 0o755);
    }

    try {
      const result = spawnSync(process.execPath, [fixture.script], {
        cwd: process.cwd(),
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${binDir}${delimiter}${process.env.PATH}`,
          DART_DECIMATE_CACHE_RECORD: recordPath,
          npm_config_cache: inheritedCache,
        },
      });

      assert.equal(result.status, 0, result.stderr);
      const usedCache = readFileSync(recordPath, "utf8");
      assert.notEqual(usedCache, inheritedCache);
      assert.equal(basename(usedCache), "npm-cache");
      assert.match(
        basename(dirname(usedCache)),
        new RegExp(`^${fixture.tempPrefix}`),
      );
      assert.equal(existsSync(usedCache), false);
      assert.equal(existsSync(inheritedCache), false);
    } finally {
      rmSync(testDir, { recursive: true, force: true });
    }
  });
}
