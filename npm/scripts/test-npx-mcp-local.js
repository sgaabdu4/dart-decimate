#!/usr/bin/env node

const { mkdtempSync, readFileSync, rmSync } = require("fs");
const { tmpdir } = require("os");
const { join } = require("path");
const { spawnSync } = require("child_process");

const root = join(__dirname, "..", "..");
const tempDir = mkdtempSync(join(tmpdir(), "dart-decimate-npx-mcp-"));

try {
  const pack = spawnSync(
    "npm",
    ["pack", "--json", "--pack-destination", tempDir],
    { cwd: root, encoding: "utf8" },
  );
  if (pack.error) {
    process.stderr.write(`failed to execute npm: ${pack.error.message}\n`);
    process.exit(pack.error.code === "ENOENT" ? 127 : 1);
  }
  if (pack.status !== 0) {
    process.stderr.write(pack.stderr || "");
    process.exit(pack.status || 1);
  }
  const [metadata] = JSON.parse(pack.stdout);
  const tarball = join(tempDir, metadata.filename);
  readFileSync(tarball);

  const initialize =
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}\n';
  const result = spawnSync(
    "npx",
    ["--yes", "--package", tarball, "--", "dart-decimate-mcp"],
    {
      input: initialize,
      encoding: "utf8",
      env: { ...process.env, DART_DECIMATE_SKIP_DOWNLOAD: "1" },
    },
  );
  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }
  if (result.error) {
    process.stderr.write(`failed to execute npx: ${result.error.message}\n`);
    process.exit(result.error.code === "ENOENT" ? 127 : 1);
  }
  if (result.status !== 0) {
    process.exit(result.status || 1);
  }
  const response = JSON.parse(result.stdout.trim());
  if (response.result?.protocolVersion !== "2025-11-25") {
    process.stderr.write("dart-decimate-mcp did not negotiate MCP 2025-11-25\n");
    process.exit(1);
  }
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
