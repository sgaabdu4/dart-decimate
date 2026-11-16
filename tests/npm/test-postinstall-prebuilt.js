#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const {
  BINARIES,
  assertSucceeded,
  buildAsset,
  executableName,
  readVersion,
  relayOutput,
  releaseAssetName,
  root,
  runWithTempRoot,
  spawnResult,
  withAssetServer,
} = require("./support.js");

runWithTempRoot("dart-decimate-postinstall-", main);

/** @param {string} tempRoot */
async function main(tempRoot) {
  const packageVersion = readVersion(path.join(root, "package.json"));
  const scriptDir = path.join(tempRoot, "npm", "scripts");
  fs.mkdirSync(scriptDir, { recursive: true });
  fs.copyFileSync(
    path.join(root, "npm", "scripts", "postinstall.js"),
    path.join(scriptDir, "postinstall.js"),
  );
  fs.writeFileSync(
    path.join(tempRoot, "package.json"),
    JSON.stringify({ name: "dart-decimate", version: packageVersion }, null, 2),
  );
  const assetPath = buildAsset(
    tempRoot,
    releaseAssetName(),
    () => "#!/usr/bin/env sh\nexit 0\n",
  );

  await withAssetServer(assetPath, packageVersion, async (baseUrl) => {
    const result = await spawnResult(
      "node",
      [path.join(scriptDir, "postinstall.js")],
      {
        cwd: tempRoot,
        env: {
          ...process.env,
          CARGO: path.join(tempRoot, "missing-cargo"),
          DART_DECIMATE_RELEASE_BASE_URL: baseUrl,
        },
      },
    );
    relayOutput(result);
    assertSucceeded(result, "postinstall");
  });
  assertBinariesInstalled(tempRoot);
}

/** @param {string} tempRoot */
function assertBinariesInstalled(tempRoot) {
  for (const binary of BINARIES) {
    const installed = path.join(
      tempRoot,
      "npm",
      "bin-cache",
      executableName(binary),
    );
    if (!fs.existsSync(installed)) {
      throw new Error(`missing installed prebuilt binary ${binary}`);
    }
  }
}
