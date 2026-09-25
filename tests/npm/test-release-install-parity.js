#!/usr/bin/env node

const { spawn, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const http = require("node:http");
const { tmpdir } = require("node:os");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const fixture = path.join(root, "tests", "fixtures", "install-parity");
const packageJson = JSON.parse(
  fs.readFileSync(path.join(root, "package.json"), "utf8"),
);
const tempRoot = fs.mkdtempSync(
  path.join(tmpdir(), "dart-decimate-release-parity-"),
);

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});

async function main() {
  try {
    const assetDir = path.resolve(
      process.env.DART_DECIMATE_RELEASE_ASSET_DIR ||
        path.join(root, "dist-assets"),
    );
    const assetName = releaseAssetName();
    const assetPath = path.join(assetDir, assetName);
    if (!isFile(assetPath)) {
      throw new Error(`missing release asset ${assetPath}`);
    }

    const cargoBinary = installWithCargo();
    const tarball = packNpmPackage();
    const projectDir = path.join(tempRoot, "npm-project");
    fs.mkdirSync(projectDir, { recursive: true });
    fs.writeFileSync(
      path.join(projectDir, "package.json"),
      JSON.stringify({ name: "release-parity-probe", private: true }, null, 2),
    );

    const server = http.createServer((request, response) => {
      if (request.url !== `/v${packageJson.version}/${assetName}`) {
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
      const { port } = /** @type {import("node:net").AddressInfo} */ (
        server.address()
      );
      await installNpmTarball(tarball, projectDir, port);
    } finally {
      await new Promise((resolve) => server.close(resolve));
    }

    const npmBinary = path.join(
      projectDir,
      "node_modules",
      ".bin",
      process.platform === "win32" ? "dart-decimate.cmd" : "dart-decimate",
    );
    assertInstalledPackageVersion(projectDir);
    assertVersion(cargoBinary, "Cargo");
    assertVersion(npmBinary, "npm");

    const cargoReport = runReport(cargoBinary, "Cargo");
    const npmReport = runReport(npmBinary, "npm");
    if (cargoReport !== npmReport) {
      throw new Error(
        "Cargo and npm installations emitted different JSON reports",
      );
    }

    const report = JSON.parse(cargoReport);
    if (
      report.schema_version !== "dart-decimate.report.v1" ||
      report.tool !== `dart-decimate ${packageJson.version}` ||
      report.verdict !== "pass" ||
      report.summary?.code_duplications !== 0 ||
      report.summary?.unrendered_widgets !== 0 ||
      report.summary?.findings !== 0 ||
      report.findings?.length !== 0
    ) {
      throw new Error(`unexpected parity report: ${cargoReport}`);
    }

    console.log(
      `release install parity ok: Cargo source and npm ${packageJson.version} emitted identical reports`,
    );
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

/** @param {string} filePath */
function isFile(filePath) {
  try {
    return fs.statSync(filePath).isFile();
  } catch {
    return false;
  }
}

function installWithCargo() {
  const cargoRoot = path.join(tempRoot, "cargo-root");
  const gitUrl = process.env.DART_DECIMATE_CARGO_GIT_URL;
  const tag = process.env.DART_DECIMATE_CARGO_TAG;
  const revision = process.env.DART_DECIMATE_CARGO_REV;
  const args = ["install", "--locked", "--force", "--root", cargoRoot];
  if (gitUrl || tag || revision) {
    if (!gitUrl || Boolean(tag) === Boolean(revision)) {
      throw new Error(
        "DART_DECIMATE_CARGO_GIT_URL and exactly one of DART_DECIMATE_CARGO_TAG or DART_DECIMATE_CARGO_REV must be set together",
      );
    }
    args.push(
      "--git",
      gitUrl,
      tag ? "--tag" : "--rev",
      /** @type {string} */ (tag || revision),
    );
    args.push("dart-decimate");
  } else {
    args.push("--path", root);
  }
  run("cargo", args, root, "Cargo install");
  return path.join(
    cargoRoot,
    "bin",
    process.platform === "win32" ? "dart-decimate.exe" : "dart-decimate",
  );
}

function packNpmPackage() {
  const result = run(
    "npm",
    ["pack", "--json", "--pack-destination", tempRoot],
    root,
    "npm pack",
  );
  const [metadata] = JSON.parse(result.stdout);
  return path.join(tempRoot, metadata.filename);
}

/** @param {string} tarball @param {string} projectDir @param {number} port */
async function installNpmTarball(tarball, projectDir, port) {
  const result = await spawnResult(
    "npm",
    ["install", "--no-audit", "--no-fund", tarball],
    {
      cwd: projectDir,
      env: {
        ...process.env,
        CARGO: path.join(tempRoot, "missing-cargo"),
        DART_DECIMATE_RELEASE_BASE_URL: `http://127.0.0.1:${port}/v${packageJson.version}`,
        npm_config_cache: path.join(tempRoot, "npm-cache"),
      },
    },
  );
  if (result.status !== 0 || result.error) {
    throw new Error(
      result.error?.message ||
        result.stderr ||
        `npm install exited ${result.status}`,
    );
  }
}

/** @param {string} projectDir */
function assertInstalledPackageVersion(projectDir) {
  const installed = JSON.parse(
    fs.readFileSync(
      path.join(projectDir, "node_modules", "dart-decimate", "package.json"),
      "utf8",
    ),
  );
  if (installed.version !== packageJson.version) {
    throw new Error(
      `npm installed dart-decimate ${installed.version}; expected ${packageJson.version}`,
    );
  }
}

/** @param {string} binary @param {string} label */
function assertVersion(binary, label) {
  const result = run(binary, ["--version"], fixture, `${label} --version`);
  const expected = `dart-decimate ${packageJson.version}`;
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

function releaseAssetName() {
  const platform = process.platform === "win32" ? "windows" : process.platform;
  const arch = process.arch === "x64" ? "x64" : process.arch;
  return `dart-decimate-${platform}-${arch}.tar.gz`;
}

/** @param {string} command @param {string[]} args @param {string} cwd @param {string} label */
function run(command, args, cwd, label) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0 || result.error) {
    throw new Error(
      result.error?.message ||
        result.stderr ||
        `${label} exited ${result.status}`,
    );
  }
  return result;
}

/** @typedef {{ error?: Error, status: number | null, stderr: string, stdout: string }} ChildResult */

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
