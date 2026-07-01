#!/usr/bin/env node

const { spawn, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const http = require("node:http");
const { tmpdir } = require("node:os");
const path = require("node:path");

if (process.platform === "win32") {
  console.log("skipping npx prebuilt fixture on Windows");
  process.exit(0);
}

const root = path.resolve(__dirname, "../..");
const tempRoot = fs.mkdtempSync(path.join(tmpdir(), "dart-decimate-npx-prebuilt-"));

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});

async function main() {
  const packageJson = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  const platform = process.platform === "win32" ? "windows" : process.platform;
  const arch = process.arch === "x64" ? "x64" : process.arch;
  const assetName = `dart-decimate-${platform}-${arch}.tar.gz`;

  const projectDir = path.join(tempRoot, "project");
  const payloadDir = path.join(tempRoot, "payload");
  const assetDir = path.join(tempRoot, "assets");
  fs.mkdirSync(projectDir, { recursive: true });
  fs.mkdirSync(payloadDir, { recursive: true });
  fs.mkdirSync(assetDir, { recursive: true });

  for (const binary of ["dart-decimate", "dart-decimate-mcp"]) {
    const binaryPath = path.join(payloadDir, binary);
    fs.writeFileSync(
      binaryPath,
      `#!/usr/bin/env sh\nprintf '${binary} prebuilt'\nfor arg in "$@"; do printf ' %s' "$arg"; done\nprintf '\\n'\n`,
    );
    fs.chmodSync(binaryPath, 0o755);
  }

  const assetPath = path.join(assetDir, assetName);
  const archive = spawnSync("tar", ["-czf", assetPath, "-C", payloadDir, "."], {
    encoding: "utf8",
  });
  if (archive.status !== 0 || archive.error) {
    throw new Error(archive.stderr || archive.error?.message || "failed to create test archive");
  }

  const pack = spawnSync("npm", ["pack", "--json", "--pack-destination", tempRoot], {
    cwd: root,
    encoding: "utf8",
  });
  if (pack.status !== 0 || pack.error) {
    throw new Error(pack.stderr || pack.error?.message || "failed to pack npm package");
  }
  const [metadata] = JSON.parse(pack.stdout);
  const tarball = path.join(tempRoot, metadata.filename);

  const server = http.createServer((request, response) => {
    if (request.url !== `/v${packageJson.version}/${assetName}`) {
      response.writeHead(404);
      response.end("not found");
      return;
    }
    response.writeHead(200, { "content-type": "application/gzip" });
    fs.createReadStream(assetPath).pipe(response);
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();

  try {
    const result = await runNpx(tarball, projectDir, {
      ...process.env,
      CARGO: path.join(tempRoot, "missing-cargo"),
      DART_DECIMATE_RELEASE_BASE_URL: `http://127.0.0.1:${port}/v${packageJson.version}`,
      npm_config_cache: path.join(tempRoot, "npm-cache"),
    });

    if (result.stdout) {
      process.stdout.write(result.stdout);
    }
    if (result.stderr) {
      process.stderr.write(result.stderr);
    }
    if (result.status !== 0 || result.error) {
      throw new Error(result.error?.message || `npx exited ${result.status}`);
    }
    if (!result.stdout.includes("dart-decimate prebuilt --help")) {
      throw new Error("npx did not execute the downloaded prebuilt binary");
    }
    if (result.stderr.includes("Rust/Cargo")) {
      throw new Error("npx attempted the Cargo fallback even though a prebuilt binary was available");
    }
  } finally {
    await new Promise((resolve) => server.close(resolve));
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function runNpx(tarball, cwd, env) {
  return new Promise((resolve) => {
    const child = spawn("npx", ["--yes", "--package", tarball, "--", "dart-decimate", "--help"], {
      cwd,
      env,
    });
    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", (error) => {
      resolve({ error, stdout, stderr, status: null });
    });
    child.on("close", (status) => {
      resolve({ stdout, stderr, status });
    });
  });
}
