#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const {
  assertSucceeded,
  buildAsset,
  packedTarball,
  readVersion,
  relayOutput,
  releaseAssetName,
  root,
  runWithTempRoot,
  spawnResult,
  withAssetServer,
} = require("./support.js");

if (process.platform === "win32") {
  console.log("skipping npx prebuilt fixture on Windows");
  process.exit(0);
}

runWithTempRoot("dart-decimate-npx-prebuilt-", main);

/** @param {string} tempRoot */
async function main(tempRoot) {
  const packageVersion = readVersion(path.join(root, "package.json"));
  const projectDir = path.join(tempRoot, "project");
  fs.mkdirSync(projectDir, { recursive: true });
  const assetPath = buildAsset(
    tempRoot,
    releaseAssetName(),
    (binary) =>
      `#!/usr/bin/env sh\nprintf '${binary} prebuilt'\nfor arg in "$@"; do printf ' %s' "$arg"; done\nprintf '\\n'\n`,
  );
  const tarball = packedTarball(tempRoot);

  await withAssetServer(assetPath, packageVersion, async (baseUrl) => {
    const result = await spawnResult(
      "npx",
      ["--yes", "--package", tarball, "--", "dart-decimate", "--help"],
      {
        cwd: projectDir,
        env: {
          ...process.env,
          CARGO: path.join(tempRoot, "missing-cargo"),
          DART_DECIMATE_RELEASE_BASE_URL: baseUrl,
          npm_config_cache: path.join(tempRoot, "npm-cache"),
        },
      },
    );
    relayOutput(result);
    assertSucceeded(result, "npx");
    assertPrebuiltRan(result);
  });
}

/** @param {import("./support.js").ChildResult} result */
function assertPrebuiltRan(result) {
  if (!result.stdout.includes("dart-decimate prebuilt --help")) {
    throw new Error("npx did not execute the downloaded prebuilt binary");
  }
  if (result.stderr.includes("Rust/Cargo")) {
    throw new Error(
      "npx attempted the Cargo fallback even though a prebuilt binary was available",
    );
  }
}
