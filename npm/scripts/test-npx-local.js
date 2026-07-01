#!/usr/bin/env node

const { mkdtempSync, readFileSync, rmSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { spawnSync } = require("node:child_process");

const root = join(__dirname, "..", "..");
const tempDir = mkdtempSync(join(tmpdir(), "dart-decimate-npx-"));

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

  const result = spawnSync(
    "npx",
    ["--yes", "--package", tarball, "--", "dart-decimate", "--help"],
    { encoding: "utf8", env: { ...process.env, DART_DECIMATE_SKIP_DOWNLOAD: "1" } },
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
  if (!result.stdout.includes("Usage: dart-decimate")) {
    process.stderr.write("dart-decimate --help did not print the expected usage\n");
    process.exit(1);
  }
} finally {
  rmSync(tempDir, { recursive: true, force: true });
}
