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
  const response = JSON.parse(result.stdout.split("\n")[0]);
  assert.equal(response.id, 1);
  assert.equal(response.result.protocolVersion, "2025-11-25");
  assert.equal(response.result.serverInfo.name, "dart-decimate-mcp");
});
