#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const skippedDirs = new Set([
  ".git",
  "target",
  "node_modules",
  "npm/bin-cache",
  ".cargo-home",
  ".rustup-home",
  ".codebase-memory",
  "coverage",
]);
const extensions = new Set([
  ".rs",
  ".js",
  ".mjs",
  ".json",
  ".toml",
  ".md",
  ".yml",
  ".yaml",
  ".sh",
]);
const explicitFiles = new Set([".gitignore", "CODEOWNERS"]);
const skippedFiles = new Set(["scripts/check-dart-decimate-migration.mjs"]);

/** @type {Array<[string, RegExp]>} */
const checks = [
  ["old scoped npm package", /@sgaabdu4\/decimate/],
  ["old GitHub repo", /sgaabdu4\/decimate/],
  ["old banner filename", /(?<!dart-)decimate-banner\.png/],
  [
    "double migrated package name",
    /dart-dart-decimate|dart_dart_decimate|DART_DART_DECIMATE/,
  ],
  ["old MCP binary", /(?<!dart-)decimate-mcp/],
  ["old suppression directive", /(?<!dart-)decimate-ignore|fallow-ignore/],
  ["old rule id prefix", /(?<!dart-)decimate\//],
  ["old schema prefix", /(?<!dart-)decimate\./],
  ["old config filename", /\.decimaterc|(?<!dart-)decimate\.toml|\.decimate\b/],
  ["old env var prefix", /(?<!DART_)DECIMATE_/],
  ["old cargo binary env", /CARGO_BIN_EXE_decimate\b/],
  ["old crate path", /use decimate::|(?<!dart_)decimate::/],
  ["old underscore identifier", /(?<!dart_)decimate_/],
  ["old CLI command", /(?<!dart[-_])\bdecimate\b/],
  ["old product name", /(?<!Dart )\bDecimate\b/],
];

const findings = [];
for (const file of walk(root)) {
  const relative = path.relative(root, file);
  const source = fs.readFileSync(file, "utf8");
  const lines = source.split(/\r?\n/);
  for (const [label, pattern] of checks) {
    for (const [index, line] of lines.entries()) {
      pattern.lastIndex = 0;
      if (pattern.test(line)) {
        findings.push(`${relative}:${index + 1}: ${label}: ${line.trim()}`);
      }
    }
  }
}

if (findings.length > 0) {
  console.error(findings.join("\n"));
  process.exit(1);
}

console.log("dart-decimate migration ok");

/** @param {string} dir @returns {Generator<string>} */
function* walk(dir) {
  for (const entry of scanEntries(dir)) {
    if (entry.directory) {
      yield* walk(entry.absolute);
    } else {
      yield entry.absolute;
    }
  }
}

/** @typedef {{ absolute: string, directory: boolean, file: boolean, relative: string }} ScanEntry */

/** @param {string} dir */
function scanEntries(dir) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .map((entry) => scanEntry(dir, entry))
    .filter(isIncludedEntry);
}

/** @param {string} dir @param {fs.Dirent} entry @returns {ScanEntry} */
function scanEntry(dir, entry) {
  const absolute = path.join(dir, entry.name);
  return {
    absolute,
    directory: entry.isDirectory(),
    file: entry.isFile(),
    relative: path.relative(root, absolute),
  };
}

/** @param {ScanEntry} entry */
function isIncludedEntry(entry) {
  if (entry.directory) {
    return !isSkipped(entry.relative);
  }
  return shouldScanFile(entry);
}

/** @param {ScanEntry} entry */
function shouldScanFile(entry) {
  if (!entry.file) {
    return false;
  }
  if (isSkippedFile(entry.relative)) {
    return false;
  }
  return isScannableName(entry.relative);
}

/** @param {string} relative */
function isSkippedFile(relative) {
  return skippedFiles.has(relative) || isLifecycleArtifact(relative);
}

/** @param {string} relative */
function isScannableName(relative) {
  return extensions.has(path.extname(relative)) || explicitFiles.has(relative);
}

/** @param {string} relative */
function isLifecycleArtifact(relative) {
  const segments = relative.split(path.sep);
  return (
    segments[0] === "features" &&
    (segments.at(-1) === "PLAN.md" || segments.includes("receipts"))
  );
}

/** @param {string} relative */
function isSkipped(relative) {
  return [...skippedDirs].some((skipped) => {
    return relative === skipped || relative.startsWith(`${skipped}${path.sep}`);
  });
}
