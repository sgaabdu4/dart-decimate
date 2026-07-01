const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

function runBinary(binaryName, args) {
  const root = path.resolve(__dirname, "../..");
  const exeName = process.platform === "win32" ? `${binaryName}.exe` : binaryName;
  const cachedBinary = path.join(root, "npm", "bin-cache", exeName);
  const releaseBinary = path.join(root, "target", "release", exeName);
  const debugBinary = path.join(root, "target", "debug", exeName);

  runFirstExisting([cachedBinary, releaseBinary, debugBinary], args);
  installCachedBinary(root);
  runFirstExisting([cachedBinary, releaseBinary, debugBinary], args);

  const cargo = process.env.CARGO || "cargo";
  run(cargo, ["run", "--release", "--locked", "--bin", binaryName, "--", ...args], root, binaryName);
}

function runFirstExisting(candidates, args) {
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      run(candidate, args, undefined, path.basename(candidate));
    }
  }
}

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

  if (result.error && result.error.code !== "ENOENT") {
    console.error(`dart-decimate: install step failed to start: ${result.error.message}`);
  }
  if (result.signal) {
    process.kill(process.pid, result.signal);
  }
}

function run(command, commandArgs, cwd = undefined, label = command) {
  const result = spawnSync(command, commandArgs, {
    cwd,
    stdio: "inherit",
    windowsHide: false,
  });

  if (result.error) {
    if (result.error.code === "ENOENT") {
      console.error(
        `${label}: Rust/Cargo is required to build the npm source package. ` +
          "Install Rust from https://rustup.rs or use a release package with a prebuilt binary.",
      );
      process.exit(127);
    }
    console.error(`${label}: failed to execute ${command}: ${result.error.message}`);
    process.exit(1);
  }

  if (result.signal) {
    process.kill(process.pid, result.signal);
    return;
  }

  process.exit(result.status ?? 1);
}

module.exports = { runBinary };
