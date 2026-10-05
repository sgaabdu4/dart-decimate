import { execFileSync } from "node:child_process";

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

/** @param {string} output */
function parseGitEnvironmentVariables(output) {
  return output
    .split("\n")
    .map((value) => value.trim())
    .filter(Boolean);
}

/** @param {NodeJS.ProcessEnv} env @param {string[]} names */
function removeGitEnvironmentVariables(env, names) {
  for (const name of names) {
    delete env[name];
  }
}

export const gitEnv = sanitizedGitEnv();

/** @param {string[]} args */
function git(args) {
  return execFileSync("git", args, {
    encoding: "utf8",
    env: gitEnv,
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
}

/** Mirrors release.yml, which accepts an existing version when its tag points at HEAD. @param {string} version */
export function releasedFromHead(version) {
  try {
    return (
      git([
        "rev-parse",
        "--verify",
        "--quiet",
        `refs/tags/v${version}^{commit}`,
      ]) === git(["rev-parse", "HEAD"]) &&
      git(["status", "--porcelain", "--untracked-files=no"]) === ""
    );
  } catch {
    return false;
  }
}

/** @param {unknown} value @param {string} key @returns {unknown} */
export function property(value, key) {
  return typeof value === "object" && value !== null
    ? new Map(Object.entries(value)).get(key)
    : undefined;
}

/** @param {unknown} error */
export function commandOutput(error) {
  return `${property(error, "stdout") ?? ""}\n${property(error, "stderr") ?? ""}`;
}
