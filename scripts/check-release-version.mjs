#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";

const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
const name = pkg.name;
const version = pkg.version;

if (process.env.DART_DECIMATE_ALLOW_EXISTING_VERSION === "1") {
  console.log(
    "release version check skipped by DART_DECIMATE_ALLOW_EXISTING_VERSION=1",
  );
  process.exit(0);
}

try {
  execFileSync("npm", ["view", `${name}@${version}`, "version"], {
    stdio: "pipe",
    encoding: "utf8",
  });
  console.error(
    `${name}@${version} is already published; bump Cargo.toml and package.json`,
  );
  process.exit(1);
} catch (error) {
  const output = `${error.stdout ?? ""}\n${error.stderr ?? ""}`;
  if (output.includes("E404") || output.includes("404 Not Found")) {
    console.log(`release version ok: ${name}@${version} is not published`);
    process.exit(0);
  }

  console.error(`could not verify npm version ${name}@${version}`);
  if (output.trim()) {
    console.error(output.trim());
  }
  process.exit(1);
}
