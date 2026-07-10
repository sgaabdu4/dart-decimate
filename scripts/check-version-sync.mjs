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

const tag =
  process.env.GITHUB_REF_TYPE === "tag" ? process.env.GITHUB_REF_NAME : "";
if (tag && tag !== `v${npmVersion}`) {
  console.error(`tag ${tag} does not match package version ${npmVersion}`);
  process.exit(1);
}

console.log(`version ok: ${npmVersion}`);
