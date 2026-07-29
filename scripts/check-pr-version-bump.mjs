#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";

const gitEnvironmentVariables = [
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_COMMON_DIR",
  "GIT_CONFIG",
  "GIT_CONFIG_COUNT",
  "GIT_CONFIG_PARAMETERS",
  "GIT_DIR",
  "GIT_GRAFT_FILE",
  "GIT_IMPLICIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_INDEX_VERSION",
  "GIT_NAMESPACE",
  "GIT_NO_REPLACE_OBJECTS",
  "GIT_OBJECT_DIRECTORY",
  "GIT_PREFIX",
  "GIT_QUARANTINE_PATH",
  "GIT_REPLACE_REF_BASE",
  "GIT_SHALLOW_FILE",
  "GIT_WORK_TREE",
];

function sanitizedGitEnv() {
  const env = { ...process.env };
  removeGitEnvironmentVariables(env, gitEnvironmentVariables);
  try {
    const reported = execFileSync("git", ["rev-parse", "--local-env-vars"], {
      encoding: "utf8",
      env,
      stdio: ["ignore", "pipe", "ignore"],
    });
    removeGitEnvironmentVariables(env, parseGitEnvironmentVariables(reported));
  } catch {
    // The baseline still covers Git's hook-scoped repository variables.
  }
  return env;
}

function parseGitEnvironmentVariables(output) {
  return output
    .split("\n")
    .map((value) => value.trim())
    .filter(Boolean);
}

function removeGitEnvironmentVariables(env, names) {
  for (const name of names) {
    delete env[name];
  }
}

const gitEnv = sanitizedGitEnv();

function exitWithError(message) {
  console.error(message);
  process.exit(1);
}

function readBaseFile(baseRef, path) {
  try {
    return execFileSync("git", ["show", `${baseRef}:${path}`], {
      encoding: "utf8",
      env: gitEnv,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    reportBaseFileError(error, baseRef, path);
  }
}

function reportBaseFileError(error, baseRef, path) {
  const output = `${error.stdout ?? ""}\n${error.stderr ?? ""}`.trim();
  if (output) {
    console.error(output);
  }
  exitWithError(`could not read ${path} from ${baseRef}`);
}

function readCargoVersion(contents, label) {
  const match = contents.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    exitWithError(`${label} is missing package version`);
  }
  return match[1];
}

function readPackageVersion(contents, label) {
  let pkg;
  try {
    pkg = JSON.parse(contents);
  } catch {
    exitWithError(`${label} is invalid JSON`);
  }

  if (typeof pkg.version !== "string" || pkg.version.length === 0) {
    exitWithError(`${label} is missing package version`);
  }
  return pkg.version;
}

function parseSemver(version, label) {
  const match = version.match(
    /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/,
  );
  if (!match) {
    exitWithError(`${label} has invalid semver: ${version}`);
  }

  return {
    major: BigInt(match[1]),
    minor: BigInt(match[2]),
    patch: BigInt(match[3]),
    prerelease: match[4] ? match[4].split(".") : [],
  };
}

function compareNumber(left, right) {
  if (left < right) {
    return -1;
  }
  if (left > right) {
    return 1;
  }
  return 0;
}

function isNumericIdentifier(value) {
  return /^(0|[1-9]\d*)$/.test(value);
}

function comparePrerelease(left, right) {
  const presence = comparePrereleasePresence(left, right);
  if (presence !== 0 || left.length === 0) {
    return presence;
  }
  return comparePrereleaseParts(left, right);
}

function comparePrereleaseParts(left, right) {
  const length = Math.max(left.length, right.length);
  for (let index = 0; index < length; index += 1) {
    const compared = comparePrereleaseIdentifier(left[index], right[index]);
    if (compared !== 0) {
      return compared;
    }
  }

  return 0;
}

function comparePrereleasePresence(left, right) {
  return Number(left.length === 0) - Number(right.length === 0);
}

function comparePrereleaseIdentifier(left, right) {
  if (left === right) {
    return 0;
  }
  const kindComparison = compareNumber(
    identifierKind(left),
    identifierKind(right),
  );
  if (kindComparison !== 0) {
    return kindComparison;
  }
  return comparePresentPrereleaseIdentifier(left, right);
}

function identifierKind(value) {
  if (value === undefined) {
    return 0;
  }
  return isNumericIdentifier(value) ? 1 : 2;
}

function comparePresentPrereleaseIdentifier(left, right) {
  if (isNumericIdentifier(left)) {
    return compareNumber(BigInt(left), BigInt(right));
  }
  return compareNumber(left, right);
}

function compareSemver(left, right, leftLabel, rightLabel) {
  const parsedLeft = parseSemver(left, leftLabel);
  const parsedRight = parseSemver(right, rightLabel);

  for (const key of ["major", "minor", "patch"]) {
    const compared = compareNumber(parsedLeft[key], parsedRight[key]);
    if (compared !== 0) {
      return compared;
    }
  }

  return comparePrerelease(parsedLeft.prerelease, parsedRight.prerelease);
}

function requireBumped(label, current, base, failures) {
  if (compareSemver(current, base, label, `base ${label}`) <= 0) {
    failures.push(`${label} version must be bumped: ${base} -> ${current}`);
  }
}

const baseRef =
  process.argv[2] ??
  (process.env.GITHUB_BASE_REF ? `origin/${process.env.GITHUB_BASE_REF}` : "");
if (!baseRef) {
  exitWithError("usage: check-pr-version-bump.mjs <base-ref>");
}

const currentCargoVersion = readCargoVersion(
  fs.readFileSync("Cargo.toml", "utf8"),
  "Cargo.toml",
);
const currentPackageVersion = readPackageVersion(
  fs.readFileSync("package.json", "utf8"),
  "package.json",
);
const baseCargoVersion = readCargoVersion(
  readBaseFile(baseRef, "Cargo.toml"),
  "base Cargo.toml",
);
const basePackageVersion = readPackageVersion(
  readBaseFile(baseRef, "package.json"),
  "base package.json",
);

const failures = [];
if (currentCargoVersion !== currentPackageVersion) {
  failures.push(
    `version mismatch: Cargo.toml=${currentCargoVersion} package.json=${currentPackageVersion}`,
  );
}
requireBumped("Cargo.toml", currentCargoVersion, baseCargoVersion, failures);
requireBumped(
  "package.json",
  currentPackageVersion,
  basePackageVersion,
  failures,
);

if (failures.length > 0) {
  for (const failure of failures) {
    console.error(failure);
  }
  process.exit(1);
}

console.log(
  `version bump ok: Cargo.toml ${baseCargoVersion} -> ${currentCargoVersion}; package.json ${basePackageVersion} -> ${currentPackageVersion}`,
);
