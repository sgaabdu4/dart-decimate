import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { closeSync, openSync } from "node:fs";
import { devNull } from "node:os";
import { resolve } from "node:path";
import { test } from "node:test";
import { cleanEnv, runNode } from "./child-process.mjs";

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

test("npm MCP wrapper answers an initialize request", () => {
  const result = spawnSync(process.execPath, ["npm/bin/dart-decimate-mcp.js"], {
    cwd: process.cwd(),
    encoding: "utf8",
    env: { ...process.env, DART_DECIMATE_SKIP_BUILD: "1" },
    input: `${JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: { protocolVersion: "2025-11-25" },
    })}\n`,
  });

  assert.equal(result.status, 0, result.stderr);
  assert.partialDeepStrictEqual(JSON.parse(result.stdout.split("\n")[0]), {
    id: 1,
    result: {
      protocolVersion: "2025-11-25",
      serverInfo: { name: "dart-decimate-mcp" },
    },
  });
});

test("npm CLI wrapper exits 0 when the binary prints help", async () => {
  const result = await runNode(
    [resolve("npm/bin/dart-decimate.js"), "--help"],
    {
      env: { ...cleanEnv, DART_DECIMATE_SKIP_BUILD: "1" },
    },
  );

  assert.equal(result.status, 0, result.stderr);
});

test("npm MCP wrapper exits 0 when the binary reads an empty input", async () => {
  closeSync(0);
  assert.equal(openSync(devNull, "r"), 0);

  const result = await runNode([resolve("npm/bin/dart-decimate-mcp.js")], {
    env: { ...cleanEnv, DART_DECIMATE_SKIP_BUILD: "1" },
  });

  assert.equal(result.status, 0, result.stderr);
});
