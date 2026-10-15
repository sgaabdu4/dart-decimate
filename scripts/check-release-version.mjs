#!/usr/bin/env node

import { execFile } from "node:child_process";
import fs from "node:fs";
import { promisify } from "node:util";
import { commandOutput, property, releasedFromHead } from "./released-head.mjs";

const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
const name = property(pkg, "name");
const version = property(pkg, "version");
if (typeof name !== "string" || typeof version !== "string") {
  console.error("package.json is missing its name or version");
  process.exit(1);
}

if (process.env.DART_DECIMATE_ALLOW_EXISTING_VERSION === "1") {
  console.log(
    "release version check skipped by DART_DECIMATE_ALLOW_EXISTING_VERSION=1",
  );
  process.exit(0);
}

try {
  await promisify(execFile)(
    "bun",
    ["pm", "view", `${name}@${version}`, "version"],
    {
      encoding: "utf8",
    },
  );
  if (releasedFromHead(version)) {
    console.log(
      `release version ok: ${name}@${version} was released from HEAD`,
    );
    process.exit(0);
  }
  console.error(
    `${name}@${version} is already published; bump Cargo.toml and package.json`,
  );
  process.exit(1);
} catch (error) {
  const output = commandOutput(error);
  if (
    output.includes("404 Not Found") ||
    /No version of .* satisfying .* found/.test(output)
  ) {
    console.log(`release version ok: ${name}@${version} is not published`);
    process.exit(0);
  }

  console.error(`could not verify npm version ${name}@${version}`);
  if (output.trim()) {
    console.error(output.trim());
  }
  process.exit(1);
}
