#!/usr/bin/env node

const { spawn, spawnSync } = require("node:child_process");
const fs = require("node:fs");
const http = require("node:http");
const { tmpdir } = require("node:os");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const tempRoot = fs.mkdtempSync(path.join(tmpdir(), "dart-decimate-postinstall-"));

main().catch((error) => {
  console.error(error.message);
  process.exit(1);
});

async function main() {
  const packageJson = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  const platform = process.platform === "win32" ? "windows" : process.platform;
  const arch = process.arch === "x64" ? "x64" : process.arch;
  const assetName = `dart-decimate-${platform}-${arch}.tar.gz`;

  const scriptDir = path.join(tempRoot, "npm", "scripts");
  const payloadDir = path.join(tempRoot, "payload");
  const assetDir = path.join(tempRoot, "assets");
  fs.mkdirSync(scriptDir, { recursive: true });
  fs.mkdirSync(payloadDir, { recursive: true });
  fs.mkdirSync(assetDir, { recursive: true });

  fs.copyFileSync(
    path.join(root, "npm", "scripts", "postinstall.js"),
    path.join(scriptDir, "postinstall.js"),
  );
  fs.writeFileSync(
    path.join(tempRoot, "package.json"),
    JSON.stringify({ name: "dart-decimate", version: packageJson.version }, null, 2),
  );

  for (const binary of ["dart-decimate", "dart-decimate-mcp"]) {
    const binaryPath = path.join(payloadDir, process.platform === "win32" ? `${binary}.exe` : binary);
    fs.writeFileSync(binaryPath, "#!/usr/bin/env sh\nexit 0\n");
    fs.chmodSync(binaryPath, 0o755);
  }

  const assetPath = path.join(assetDir, assetName);
  const archive = spawnSync("tar", ["-czf", assetPath, "-C", payloadDir, "."], {
    encoding: "utf8",
  });
  if (archive.status !== 0 || archive.error) {
    throw new Error(archive.stderr || archive.error?.message || "failed to create test archive");
  }

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
    const result = await runPostinstall(path.join(scriptDir, "postinstall.js"), {
      cwd: tempRoot,
      env: {
        ...process.env,
        CARGO: path.join(tempRoot, "missing-cargo"),
        DART_DECIMATE_RELEASE_BASE_URL: `http://127.0.0.1:${port}/v${packageJson.version}`,
      },
    });

    if (result.stdout) {
      process.stdout.write(result.stdout);
    }
    if (result.stderr) {
      process.stderr.write(result.stderr);
    }
    if (result.status !== 0 || result.error) {
      throw new Error(result.error?.message || `postinstall exited ${result.status}`);
    }

    for (const binary of ["dart-decimate", "dart-decimate-mcp"]) {
      const installed = path.join(
        tempRoot,
        "npm",
        "bin-cache",
        process.platform === "win32" ? `${binary}.exe` : binary,
      );
      if (!fs.existsSync(installed)) {
        throw new Error(`missing installed prebuilt binary ${binary}`);
      }
    }
  } finally {
    await new Promise((resolve) => server.close(resolve));
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function runPostinstall(script, options) {
  return new Promise((resolve) => {
    const child = spawn("node", [script], options);
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
