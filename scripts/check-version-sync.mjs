#!/usr/bin/env node

import fs from "node:fs";

// The crate name is compared rather than interpolated into a pattern, so a
// metacharacter in it cannot change the match. CRLF is normalised first.
function field(block, key) {
  return block.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"$`, "m"))?.[1];
}

function cargoLockVersion(contents, crate) {
  const blocks = contents.replace(/\r\n/g, "\n").split("\n\n");
  const entry = blocks.find((block) => field(block, "name") === crate);
  return entry && field(entry, "version");
}

const cargo = fs.readFileSync("Cargo.toml", "utf8");
const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
const cargoMatch = cargo.match(/^version\s*=\s*"([^"]+)"/m);

if (!cargoMatch) {
  console.error("Cargo.toml is missing package version");
  process.exit(1);
}

const cargoVersion = cargoMatch[1];
const npmVersion = pkg.version;

if (cargoVersion !== npmVersion) {
  console.error(
    `version mismatch: Cargo.toml=${cargoVersion} package.json=${npmVersion}`,
  );
  process.exit(1);
}

// Lockfiles carry the version too, so leaving them out lets a manifest bump
// ship with a stale lock that nothing downstream re-checks.
const crate = cargo.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
const lockVersions = [];

if (fs.existsSync("package-lock.json")) {
  const lock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
  lockVersions.push(["package-lock.json", pkg.name, lock.version]);
  lockVersions.push([
    'package-lock.json packages[""]',
    pkg.name,
    lock.packages?.[""]?.version,
  ]);
}

if (fs.existsSync("Cargo.lock")) {
  if (!crate) {
    console.error("Cargo.toml is missing package name");
    process.exit(1);
  }
  const lock = fs.readFileSync("Cargo.lock", "utf8");
  lockVersions.push(["Cargo.lock", crate, cargoLockVersion(lock, crate)]);
}

for (const [source, name, version] of lockVersions) {
  if (version === undefined) {
    console.error(`${source} is missing the ${name} version`);
    process.exit(1);
  }
  if (version !== npmVersion) {
    console.error(
      `version mismatch: ${source}=${version} expected=${npmVersion}`,
    );
    process.exit(1);
  }
}

const tag =
  process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME : "";
if (tag && tag !== `v${npmVersion}`) {
  console.error(`tag ${tag} does not match package version ${npmVersion}`);
  process.exit(1);
}

console.log(`version ok: ${npmVersion}`);
