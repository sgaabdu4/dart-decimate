#!/usr/bin/env node

const fs = require("node:fs");
const { tmpdir } = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const {
  assertSucceeded,
  executableName,
  property,
  readVersion,
  releaseAssetName,
  root,
  spawnResult,
  withAssetServer,
} = require("./support.js");

const fixture = path.join(root, "tests", "fixtures", "install-parity");
const packageVersion = readVersion(path.join(root, "package.json"));
const tempRoot = fs.mkdtempSync(
  path.join(tmpdir(), "dart-decimate-release-parity-"),
);

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});

async function main() {
  try {
    const assetPath = releaseAssetPath();
    const cargoBinary = installWithCargo();
    const tarball = packNpmPackage();
    const projectDir = path.join(tempRoot, "npm-project");
    fs.mkdirSync(projectDir, { recursive: true });
    fs.writeFileSync(
      path.join(projectDir, "package.json"),
      JSON.stringify({ name: "release-parity-probe", private: true }, null, 2),
    );

    await withAssetServer(assetPath, packageVersion, (baseUrl) =>
      installNpmTarball(tarball, projectDir, baseUrl),
    );

    assertInstalledPackageVersion(projectDir);
    assertVersion(cargoBinary, "Cargo");
    const npmBinary = path.join(
      projectDir,
      "node_modules",
      ".bin",
      process.platform === "win32" ? "dart-decimate.cmd" : "dart-decimate",
    );
    assertVersion(npmBinary, "npm");
    assertIdenticalPassingReports(cargoBinary, npmBinary);
    console.log(
      `release install parity ok: Cargo source and npm ${packageVersion} emitted identical reports`,
    );
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function releaseAssetPath() {
  const assetDir = path.resolve(
    process.env.DART_DECIMATE_RELEASE_ASSET_DIR ||
      path.join(root, "dist-assets"),
  );
  const assetPath = path.join(assetDir, releaseAssetName());
  if (!fs.statSync(assetPath, { throwIfNoEntry: false })?.isFile()) {
    throw new Error(`missing release asset ${assetPath}`);
  }
  return assetPath;
}

/** @param {string | undefined} gitUrl @param {unknown[]} references @returns {string} */
function requireGitSource(gitUrl, references) {
  if (!gitUrl || references.length !== 1) {
    throw new Error(
      "DART_DECIMATE_CARGO_GIT_URL and exactly one of DART_DECIMATE_CARGO_TAG or DART_DECIMATE_CARGO_REV must be set together",
    );
  }
  return gitUrl;
}

/** @param {string} cargoRoot */
function cargoSourceArgs(cargoRoot) {
  const gitUrl = process.env.DART_DECIMATE_CARGO_GIT_URL;
  /** @type {[string, string | undefined][]} */
  const candidates = [
    ["--tag", process.env.DART_DECIMATE_CARGO_TAG],
    ["--rev", process.env.DART_DECIMATE_CARGO_REV],
  ];
  const references = candidates.flatMap(([flag, value]) =>
    value ? [[flag, value]] : [],
  );
  const args = ["install", "--locked", "--force", "--root", cargoRoot];
  if (!gitUrl && references.length === 0) {
    return [...args, "--path", root];
  }
  return [
    ...args,
    "--git",
    requireGitSource(gitUrl, references),
    ...references[0],
    "dart-decimate",
  ];
}

function installWithCargo() {
  const cargoRoot = path.join(tempRoot, "cargo-root");
  run("cargo", cargoSourceArgs(cargoRoot), root, "Cargo install");
  return path.join(cargoRoot, "bin", executableName("dart-decimate"));
}

function packNpmPackage() {
  const result = run(
    "bun",
    ["pm", "pack", "--quiet", "--destination", tempRoot],
    root,
    "bun pm pack",
  );
  const filename = result.stdout.trim();
  if (!filename) {
    throw new Error("bun pm pack did not report a tarball filename");
  }
  return path.resolve(tempRoot, filename);
}

/** @param {string} tarball @param {string} projectDir @param {string} baseUrl */
async function installNpmTarball(tarball, projectDir, baseUrl) {
  const result = await spawnResult(
    "npm",
    ["install", "--no-audit", "--no-fund", tarball],
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
  assertSucceeded(result, "npm install");
}

/** @param {string} projectDir */
function assertInstalledPackageVersion(projectDir) {
  const installedVersion = readVersion(
    path.join(projectDir, "node_modules", "dart-decimate", "package.json"),
  );
  if (installedVersion !== packageVersion) {
    throw new Error(
      `npm installed dart-decimate ${installedVersion}; expected ${packageVersion}`,
    );
  }
}

/** @param {string} binary @param {string} label */
function assertVersion(binary, label) {
  const result = run(binary, ["--version"], fixture, `${label} --version`);
  const expected = `dart-decimate ${packageVersion}`;
  if (result.stdout.trim() !== expected) {
    throw new Error(
      `${label} reported ${result.stdout.trim()}; expected ${expected}`,
    );
  }
}

/** @param {string} binary @param {string} label */
function runReport(binary, label) {
  return run(
    binary,
    ["check", "lib", "--threshold", "0", "--format", "json"],
    fixture,
    `${label} report`,
  ).stdout;
}

/** @param {string} cargoBinary @param {string} npmBinary */
function assertIdenticalPassingReports(cargoBinary, npmBinary) {
  const cargoReport = runReport(cargoBinary, "Cargo");
  if (cargoReport !== runReport(npmBinary, "npm")) {
    throw new Error(
      "Cargo and npm installations emitted different JSON reports",
    );
  }
  if (!isPassingReport(JSON.parse(cargoReport))) {
    throw new Error(`unexpected parity report: ${cargoReport}`);
  }
}

/** @param {string} command @param {string[]} args @param {string} cwd @param {string} label */
function run(command, args, cwd, label) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  assertSucceeded(result, label);
  return result;
}

/** @param {unknown} report */
function isPassingReport(report) {
  const summary = property(report, "summary");
  const actual = [
    property(report, "schema_version"),
    property(report, "tool"),
    property(report, "verdict"),
    property(summary, "code_duplications"),
    property(summary, "unrendered_widgets"),
    property(summary, "findings"),
    property(report, "findings"),
  ];
  const expected = [
    "dart-decimate.report.v1",
    `dart-decimate ${packageVersion}`,
    "pass",
    0,
    0,
    0,
    [],
  ];
  return JSON.stringify(actual) === JSON.stringify(expected);
}
