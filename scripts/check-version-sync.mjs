#!/usr/bin/env node

import fs from "node:fs";

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
const name = pkg.name;
const lockVersions = [];

if (fs.existsSync("package-lock.json")) {
  const lock = JSON.parse(fs.readFileSync("package-lock.json", "utf8"));
  lockVersions.push(["package-lock.json", lock.version]);
  lockVersions.push([
    'package-lock.json packages[""]',
    lock.packages?.[""]?.version,
  ]);
}

if (fs.existsSync("Cargo.lock")) {
  const lock = fs.readFileSync("Cargo.lock", "utf8");
  const entry = lock.match(
    new RegExp(`\\[\\[package\\]\\]\\nname = "${name}"\\nversion = "([^"]+)"`),
  );
  lockVersions.push(["Cargo.lock", entry?.[1]]);
}

for (const [source, version] of lockVersions) {
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
