#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const http = require("node:http");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");
const { pipeline } = require("node:stream/promises");

if (process.env.DART_DECIMATE_SKIP_BUILD === "1") {
  process.exit(0);
}

const root = path.resolve(__dirname, "../..");
const exeExt = process.platform === "win32" ? ".exe" : "";
const cargo = process.env.CARGO || "cargo";
const cacheDir = path.join(root, "npm", "bin-cache");
const packageJson = JSON.parse(
  fs.readFileSync(path.join(root, "package.json"), "utf8"),
);
const releaseBaseUrl =
  process.env.DART_DECIMATE_RELEASE_BASE_URL ||
  `https://github.com/sgaabdu4/dart-decimate/releases/download/v${packageJson.version}`;

install()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(`dart-decimate: install failed: ${error.message}`);
    process.exit(1);
  });

async function install() {
  fs.mkdirSync(cacheDir, { recursive: true });

  let prebuiltError = null;
  if (process.env.DART_DECIMATE_SKIP_DOWNLOAD !== "1") {
    try {
      if (await installPrebuilt()) {
        return;
      }
    } catch (error) {
      prebuiltError = error;
    }
  }

  buildFromSource(prebuiltError);
}

async function installPrebuilt() {
  const assetName = prebuiltAssetName();
  if (!assetName) {
    return false;
  }

  const archivePath = path.join(os.tmpdir(), assetName);
  const url = `${releaseBaseUrl.replace(/\/$/, "")}/${assetName}`;
  await download(url, archivePath, 0);
  extractArchive(assetName, archivePath);
  activateCachedBinaries(assetName);
  return true;
}

function extractArchive(assetName, archivePath) {
  const extract = spawnSync("tar", ["-xzf", archivePath, "-C", cacheDir], {
    stdio: "pipe",
    windowsHide: false,
  });
  fs.rmSync(archivePath, { force: true });

  if (extract.error) {
    throw new Error(`could not extract ${assetName}: ${extract.error.message}`);
  }
  if (extract.status !== 0) {
    const stderr = extract.stderr?.toString().trim();
    throw new Error(
      `could not extract ${assetName}${stderr ? `: ${stderr}` : ""}`,
    );
  }
}

function activateCachedBinaries(assetName) {
  for (const binary of ["dart-decimate", "dart-decimate-mcp"]) {
    const cachedBinary = path.join(cacheDir, `${binary}${exeExt}`);
    if (!fs.existsSync(cachedBinary)) {
      throw new Error(`${assetName} did not contain ${binary}${exeExt}`);
    }
    if (process.platform !== "win32") {
      fs.chmodSync(cachedBinary, 0o755);
    }
  }
}

function prebuiltAssetName() {
  const platform = prebuiltPlatform();
  const arch = prebuiltArchitecture();

  if (!platform || !arch) {
    return null;
  }
  if (!supportsPrebuiltTarget(platform, arch)) {
    return null;
  }

  return `dart-decimate-${platform}-${arch}.tar.gz`;
}

function prebuiltPlatform() {
  return new Map([
    ["darwin", "darwin"],
    ["linux", "linux"],
    ["win32", "windows"],
  ]).get(process.platform);
}

function prebuiltArchitecture() {
  return new Set(["x64", "arm64"]).has(process.arch) ? process.arch : null;
}

function supportsPrebuiltTarget(platform, arch) {
  return !new Set(["linux:arm64", "windows:arm64"]).has(`${platform}:${arch}`);
}

function download(url, destination, redirectCount) {
  if (redirectCount > 5) {
    return Promise.reject(
      new Error(`too many redirects while downloading ${url}`),
    );
  }

  return new Promise((resolve, reject) => {
    const client = url.startsWith("http://") ? http : https;
    const request = client.get(url, (response) =>
      handleDownloadResponse(
        response,
        url,
        destination,
        redirectCount,
        resolve,
        reject,
      ),
    );
    request.on("error", reject);
  });
}

function handleDownloadResponse(
  response,
  url,
  destination,
  redirectCount,
  resolve,
  reject,
) {
  if (isRedirect(response)) {
    response.resume();
    const nextUrl = new URL(response.headers.location, url).toString();
    download(nextUrl, destination, redirectCount + 1).then(resolve, reject);
    return;
  }
  if (response.statusCode !== 200) {
    response.resume();
    reject(new Error(`download ${url} returned HTTP ${response.statusCode}`));
    return;
  }
  pipeline(response, fs.createWriteStream(destination, { mode: 0o600 })).then(
    resolve,
    reject,
  );
}

function isRedirect(response) {
  return (
    response.statusCode >= 300 &&
    response.statusCode < 400 &&
    Boolean(response.headers.location)
  );
}

function buildFromSource(prebuiltError) {
  const build = spawnSync(cargo, ["build", "--release", "--locked"], {
    cwd: root,
    stdio: "inherit",
    windowsHide: false,
  });
  handleBuildStartError(build.error, prebuiltError);
  if (build.status !== 0) {
    process.exit(build.status ?? 1);
  }
  cacheBuiltBinaries();
}

function handleBuildStartError(error, prebuiltError) {
  if (!error) {
    return;
  }
  if (error.code === "ENOENT") {
    if (prebuiltError) {
      console.error(
        `dart-decimate: prebuilt install failed: ${prebuiltError.message}`,
      );
    }
    console.error(
      "dart-decimate: Rust/Cargo is required as a fallback because a prebuilt binary " +
        "could not be installed for this platform. Install Rust from https://rustup.rs.",
    );
    process.exit(127);
  }
  console.error(`dart-decimate: cargo build failed to start: ${error.message}`);
  process.exit(1);
}

function cacheBuiltBinaries() {
  for (const binary of ["dart-decimate", "dart-decimate-mcp"]) {
    const builtBinary = path.join(
      root,
      "target",
      "release",
      `${binary}${exeExt}`,
    );
    const cachedBinary = path.join(cacheDir, `${binary}${exeExt}`);
    fs.copyFileSync(builtBinary, cachedBinary);
    if (process.platform !== "win32") {
      fs.chmodSync(cachedBinary, 0o755);
    }
  }
}
