const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

/** @param {string} binaryName @param {string[]} args */
function runBinary(binaryName, args) {
  const root = path.resolve(__dirname, "../..");
  const exeName =
    process.platform === "win32" ? `${binaryName}.exe` : binaryName;
  const cachedBinary = path.join(root, "npm", "bin-cache", exeName);
  const releaseBinary = path.join(root, "target", "release", exeName);
  const debugBinary = path.join(root, "target", "debug", exeName);

  runFirstExisting([cachedBinary, releaseBinary, debugBinary], args);
  installCachedBinary(root);
  runFirstExisting([cachedBinary, releaseBinary, debugBinary], args);

  const cargo = process.env.CARGO || "cargo";
  run(
    cargo,
    ["run", "--release", "--locked", "--bin", binaryName, "--", ...args],
    root,
    binaryName,
  );
}

/** @param {string[]} candidates @param {string[]} args */
function runFirstExisting(candidates, args) {
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      run(candidate, args, undefined, path.basename(candidate));
    }
  }
}

/** @param {string} root */
function installCachedBinary(root) {
  if (process.env.DART_DECIMATE_SKIP_BUILD === "1") {
    return;
  }

  const installer = path.join(root, "npm", "scripts", "postinstall.js");
  if (!fs.existsSync(installer)) {
    return;
  }

  const result = spawnSync(process.execPath, [installer], {
    cwd: root,
    stdio: "inherit",
    windowsHide: false,
  });
  handleInstallerResult(result);
}

/** @param {import("node:child_process").SpawnSyncReturns<Buffer>} result */
function handleInstallerResult(result) {
  /** @type {NodeJS.ErrnoException | undefined} */
  const error = result.error;
  if (error && error.code !== "ENOENT") {
    console.error(
      `dart-decimate: install step failed to start: ${error.message}`,
    );
  }
  forwardSignal(result.signal);
}

/** @param {string} command @param {string[]} commandArgs @param {string | undefined} [cwd] @param {string} [label] */
function run(command, commandArgs, cwd = undefined, label = command) {
  const result = spawnSync(command, commandArgs, {
    cwd,
    stdio: "inherit",
    windowsHide: false,
  });

  if (result.error) {
    handleRunError(result.error, command, label);
  }

  if (forwardSignal(result.signal)) {
    return;
  }

  process.exit(result.status ?? 1);
}

/** @param {NodeJS.ErrnoException} error @param {string} command @param {string} label */
function handleRunError(error, command, label) {
  if (error.code === "ENOENT") {
    console.error(
      `${label}: Rust/Cargo is required to build the npm source package. ` +
        "Install Rust from https://rustup.rs or use a release package with a prebuilt binary.",
    );
    process.exit(127);
  }
  console.error(`${label}: failed to execute ${command}: ${error.message}`);
  process.exit(1);
}

/** @param {NodeJS.Signals | null} signal */
function forwardSignal(signal) {
  if (!signal) {
    return false;
  }
  process.kill(process.pid, signal);
  return true;
}

module.exports = { runBinary };
