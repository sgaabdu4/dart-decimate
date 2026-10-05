import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, delimiter, dirname, join } from "node:path";
import { test } from "node:test";

test("local npx test initializes MCP cold then checks CLI with one disposable npm cache", () => {
  const testDir = mkdtempSync(join(tmpdir(), "dart-decimate-cache-test-"));
  const binDir = join(testDir, "bin");
  const recordPath = join(testDir, "cache-path.txt");
  const inheritedCache = join(testDir, "inherited-cache");
  mkdirSync(binDir);

  const fakeNpxSource = `
const { appendFileSync, readFileSync, writeFileSync } = require("node:fs");
appendFileSync(process.env.DART_DECIMATE_CACHE_RECORD, JSON.stringify({
  cache: process.env.npm_config_cache || "",
  cwd: process.cwd(),
  args: process.argv.slice(2),
  input: readFileSync(0, "utf8"),
  skipDownload: process.env.DART_DECIMATE_SKIP_DOWNLOAD,
}) + "\\n");
writeFileSync("mcp.json", '{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25"}}\\n');
writeFileSync("cli.txt", "Usage: dart-decimate\\n");
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
    const pathKey =
      Object.keys(process.env).find((key) => key.toLowerCase() === "path") ??
      "PATH";
    const result = spawnSync(
      process.execPath,
      ["tests/npm/test-npx-local.js"],
      {
        cwd: process.cwd(),
        encoding: "utf8",
        env: {
          ...process.env,
          [pathKey]: [binDir, process.env[pathKey]]
            .filter(Boolean)
            .join(delimiter),
          DART_DECIMATE_CACHE_RECORD: recordPath,
          npm_config_cache: inheritedCache,
        },
      },
    );

    assert.equal(result.status, 0, result.stderr);
    const calls = readFileSync(recordPath, "utf8")
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line));
    assert.equal(calls.length, 1);
    const [call] = calls;
    assert.ok(call instanceof Object);
    const {
      args,
      input,
      cache: usedCache,
      cwd,
      skipDownload,
    } = Object.fromEntries(Object.entries(call));
    assert.ok(Array.isArray(args));
    assert.ok(typeof input === "string");
    assert.ok(typeof usedCache === "string");
    assert.deepEqual(args.slice(0, 2), ["--yes", "--package"]);
    assert.deepEqual(args.slice(3), [
      "--call",
      "dart-decimate-mcp > mcp.json && dart-decimate --help > cli.txt",
    ]);
    assert.deepEqual(JSON.parse(input), {
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: { protocolVersion: "2025-11-25" },
    });
    assert.equal(skipDownload, "1");
    assert.equal(
      cwd,
      join(realpathSync(tmpdir()), basename(dirname(usedCache))),
    );
    assert.notEqual(usedCache, inheritedCache);
    assert.equal(basename(usedCache), "npm-cache");
    assert.match(basename(dirname(usedCache)), /^dart-decimate-npx-/);
    assert.equal(existsSync(usedCache), false);
    assert.equal(existsSync(inheritedCache), false);
  } finally {
    rmSync(testDir, { recursive: true, force: true });
  }
});
