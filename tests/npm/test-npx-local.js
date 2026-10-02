#!/usr/bin/env node

const { mkdtempSync, readFileSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { spawnSync } = require("node:child_process");

const root = join(__dirname, "..", "..");
const tempDir = mkdtempSync(join(tmpdir(), "dart-decimate-npx-"));
process.once("exit", () => rmSync(tempDir, { recursive: true, force: true }));
const npmEnv = {
  ...process.env,
  npm_config_cache: join(tempDir, "npm-cache"),
};

const pack = spawnSync(
  "npm",
  ["pack", "--json", "--pack-destination", tempDir],
  { cwd: root, encoding: "utf8", env: npmEnv },
);
if (pack.error) {
  process.stderr.write(`failed to execute npm: ${pack.error.message}\n`);
  process.exit("code" in pack.error && pack.error.code === "ENOENT" ? 127 : 1);
}
if (pack.status !== 0) {
  process.stderr.write(pack.stderr || "");
  process.exit(pack.status || 1);
}
const tarball = join(tempDir, packedFilename(pack.stdout));
readFileSync(tarball);

const initialize =
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}\n';
const result = spawnSync(
  "npx",
  [
    "--yes",
    "--package",
    tarball,
    "--call",
    "dart-decimate-mcp > mcp.json && dart-decimate --help > cli.txt",
  ],
  {
    cwd: tempDir,
    input: initialize,
    encoding: "utf8",
    env: { ...npmEnv, DART_DECIMATE_SKIP_DOWNLOAD: "1" },
  },
);
if (result.stderr) {
  process.stderr.write(result.stderr);
}
if (result.error) {
  process.stderr.write(`failed to execute npx: ${result.error.message}\n`);
  process.exit(
    "code" in result.error && result.error.code === "ENOENT" ? 127 : 1,
  );
}
if (result.status !== 0) {
  process.exit(result.status || 1);
}
if (result.stdout !== "") {
  throw new Error("npx emitted unexpected installation output on stdout");
}
const mcpOutput = readFileSync(join(tempDir, "mcp.json"), "utf8");
if (!negotiatedProtocol(JSON.parse(mcpOutput.trim()))) {
  process.stderr.write("dart-decimate-mcp did not negotiate MCP 2025-11-25\n");
  process.exit(1);
}
process.stdout.write(mcpOutput);
const cliOutput = readFileSync(join(tempDir, "cli.txt"), "utf8");
if (!cliOutput.includes("Usage: dart-decimate")) {
  process.stderr.write(
    "dart-decimate --help did not print the expected usage\n",
  );
  process.exit(1);
}
process.stdout.write(cliOutput);

/** @param {string} stdout */
function packedFilename(stdout) {
  const packed = JSON.parse(stdout);
  const metadata = Array.isArray(packed) ? packed[0] : undefined;
  if (
    typeof metadata === "object" &&
    metadata !== null &&
    "filename" in metadata &&
    typeof metadata.filename === "string"
  ) {
    return metadata.filename;
  }
  throw new Error("npm pack did not report a tarball filename");
}

/** @param {unknown} response */
function negotiatedProtocol(response) {
  if (
    typeof response !== "object" ||
    response === null ||
    !("result" in response)
  ) {
    return false;
  }
  const { result } = response;
  return (
    typeof result === "object" &&
    result !== null &&
    "protocolVersion" in result &&
    result.protocolVersion === "2025-11-25"
  );
}
