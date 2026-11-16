const { spawn, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const http = require("node:http");
const { tmpdir } = require("node:os");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const BINARIES = ["dart-decimate", "dart-decimate-mcp"];

/** @typedef {{ error?: Error, status: number | null, stderr: string, stdout: string }} ChildResult */

/** @param {unknown} value @param {string | number} key @returns {unknown} */
function property(value, key) {
  return typeof value === "object" && value !== null
    ? Reflect.get(value, key)
    : undefined;
}

/** @param {string} file */
function readVersion(file) {
  const version = property(
    JSON.parse(fs.readFileSync(file, "utf8")),
    "version",
  );
  if (typeof version === "string") {
    return version;
  }
  throw new Error(`${file} has no version`);
}

/** @param {string} stdout */
function packedFilename(stdout) {
  const filename = property(property(JSON.parse(stdout), 0), "filename");
  if (typeof filename === "string") {
    return filename;
  }
  throw new Error("npm pack did not report a tarball filename");
}

/** @param {string} destination @param {NodeJS.ProcessEnv} [env] */
function packWithNpm(destination, env = process.env) {
  return spawnSync(
    "npm",
    ["pack", "--json", "--pack-destination", destination],
    {
      cwd: root,
      encoding: "utf8",
      env,
    },
  );
}

/** @param {string} destination */
function packedTarball(destination) {
  const pack = packWithNpm(destination);
  assertSucceeded(pack, "npm pack");
  return path.join(destination, packedFilename(pack.stdout));
}

/** @param {string} binary */
function executableName(binary) {
  return process.platform === "win32" ? `${binary}.exe` : binary;
}

function releaseAssetName() {
  const platform = process.platform === "win32" ? "windows" : process.platform;
  const arch = process.arch === "x64" ? "x64" : process.arch;
  return `dart-decimate-${platform}-${arch}.tar.gz`;
}

/** @param {ChildResult} result @param {string} label */
function failureText(result, label) {
  return (
    [result.error?.message, result.stderr].find(Boolean) ??
    `${label} exited ${result.status}`
  );
}

/** @param {ChildResult} result @param {string} label */
function assertSucceeded(result, label) {
  if (result.status === 0 && !result.error) {
    return;
  }
  throw new Error(failureText(result, label));
}

/** @param {ChildResult} result */
function relayOutput(result) {
  if (result.stdout) {
    process.stdout.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }
}

/** @param {string} command @param {string[]} args @param {import("node:child_process").SpawnOptionsWithoutStdio} options @returns {Promise<ChildResult>} */
function spawnResult(command, args, options) {
  return new Promise((resolve) => {
    const child = spawn(command, args, options);
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      resolve({ error, status: null, stderr, stdout });
    });
    child.on("close", (status) => {
      resolve({ status, stderr, stdout });
    });
  });
}

/** @param {string} tempRoot @param {string} assetName @param {(binary: string) => string} scriptFor */
function buildAsset(tempRoot, assetName, scriptFor) {
  const payloadDir = path.join(tempRoot, "payload");
  const assetDir = path.join(tempRoot, "assets");
  fs.mkdirSync(payloadDir, { recursive: true });
  fs.mkdirSync(assetDir, { recursive: true });
  for (const binary of BINARIES) {
    const binaryPath = path.join(payloadDir, executableName(binary));
    fs.writeFileSync(binaryPath, scriptFor(binary));
    fs.chmodSync(binaryPath, 0o755);
  }
  const assetPath = path.join(assetDir, assetName);
  const archive = spawnSync("tar", ["-czf", assetPath, "-C", payloadDir, "."], {
    encoding: "utf8",
  });
  assertSucceeded(archive, "tar");
  return assetPath;
}

/** @param {string} assetPath @param {string} version @param {(baseUrl: string) => Promise<void>} run */
async function withAssetServer(assetPath, version, run) {
  const route = `/v${version}/${path.basename(assetPath)}`;
  const server = http.createServer((request, response) => {
    if (request.url !== route) {
      response.writeHead(404);
      response.end("not found");
      return;
    }
    response.writeHead(200, { "content-type": "application/gzip" });
    fs.createReadStream(assetPath).pipe(response);
  });
  await new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => resolve(undefined)),
  );
  try {
    const address = server.address();
    if (address === null || typeof address === "string") {
      throw new Error("test server is not listening on a TCP port");
    }
    await run(`http://127.0.0.1:${address.port}/v${version}`);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

/** @param {string} prefix @param {(tempRoot: string) => Promise<void>} main */
function runWithTempRoot(prefix, main) {
  const tempRoot = fs.mkdtempSync(path.join(tmpdir(), prefix));
  main(tempRoot)
    .catch((error) => {
      console.error(error.message);
      process.exitCode = 1;
    })
    .finally(() => fs.rmSync(tempRoot, { recursive: true, force: true }));
}

module.exports = {
  BINARIES,
  assertSucceeded,
  buildAsset,
  executableName,
  packWithNpm,
  packedFilename,
  packedTarball,
  property,
  readVersion,
  relayOutput,
  releaseAssetName,
  root,
  runWithTempRoot,
  spawnResult,
  withAssetServer,
};
