import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { gzipSync } from "node:zlib";
import { cleanEnv, runNode, withServer } from "./child-process.mjs";

const postinstall = resolve("npm/scripts/postinstall.js");
const emptyTarball = gzipSync(Buffer.alloc(1024));

/** @param {import("node:http").RequestListener} handler @param {NodeJS.ProcessEnv} [env] */
function install(handler, env = {}) {
  const temp = mkdtempSync(join(tmpdir(), "dart-decimate-postinstall-"));
  return withServer(handler, (baseUrl) =>
    runNode([postinstall], {
      env: {
        ...cleanEnv,
        CARGO: join(temp, "missing-cargo"),
        DART_DECIMATE_RELEASE_BASE_URL: baseUrl,
        TMPDIR: temp,
        ...env,
      },
    }),
  );
}

test("postinstall reports a missing release asset and the Rust fallback", async () => {
  const result = await install((_request, response) => {
    response.writeHead(404);
    response.end();
  });

  assert.equal(result.status, 127);
  assert.match(
    result.stderr,
    /prebuilt install failed: download http:\/\/127\.0\.0\.1:\d+\/dart-decimate-\S+\.tar\.gz returned HTTP 404/,
  );
  assert.match(result.stderr, /Rust\/Cargo is required as a fallback/);
});

test("postinstall follows a release redirect", async () => {
  const result = await install((request, response) => {
    if (request.url === "/moved.tar.gz") {
      response.writeHead(404);
    } else {
      response.writeHead(302, { location: "/moved.tar.gz" });
    }
    response.end();
  });

  assert.equal(result.status, 127);
  assert.match(result.stderr, /\/moved\.tar\.gz returned HTTP 404/);
});

test("postinstall stops after five redirects", async () => {
  let hops = 0;
  const result = await install((_request, response) => {
    hops += 1;
    response.writeHead(302, { location: `/hop-${hops}` });
    response.end();
  });

  assert.equal(result.status, 127);
  assert.equal(hops, 6);
  assert.match(
    result.stderr,
    /too many redirects while downloading \S+\/hop-6/,
  );
});

test("postinstall rejects an asset that is not a gzip tarball", async () => {
  const result = await install((_request, response) => {
    response.writeHead(200);
    response.end("not a tarball");
  });

  assert.equal(result.status, 127);
  assert.match(result.stderr, /could not extract dart-decimate-\S+\.tar\.gz/);
});

test("postinstall rejects an asset without the CLI binaries", async () => {
  const result = await install((_request, response) => {
    response.writeHead(200);
    response.end(emptyTarball);
  });

  assert.equal(result.status, 127);
  assert.match(result.stderr, /did not contain dart-decimate/);
});

test("postinstall exits with the source build's status when Cargo fails", async () => {
  const temp = mkdtempSync(join(tmpdir(), "dart-decimate-cargo-"));
  const cargo = join(temp, "cargo");
  writeFileSync(cargo, "#!/bin/sh\nexit 3\n");
  chmodSync(cargo, 0o755);

  const result = await install(
    (_request, response) => {
      response.writeHead(500);
      response.end();
    },
    { CARGO: cargo, DART_DECIMATE_SKIP_DOWNLOAD: "1" },
  );

  assert.equal(result.status, 3);
  assert.doesNotMatch(result.stderr, /prebuilt install failed/);
});
