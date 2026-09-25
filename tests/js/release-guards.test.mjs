import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { cleanEnv, runNode, withServer } from "./child-process.mjs";

const versionSync = resolve("scripts/check-version-sync.mjs");
const versionBump = resolve("scripts/check-pr-version-bump.mjs");
const releaseVersion = resolve("scripts/check-release-version.mjs");
const packageVersion = JSON.parse(readFileSync("package.json", "utf8")).version;

/** @param {{ cargo: string, npm: string, lock?: string }} versions */
function project({ cargo, npm, lock = cargo }) {
  const root = mkdtempSync(join(tmpdir(), "dart-decimate-release-guard-"));
  writeVersions(root, cargo, npm);
  writeFileSync(
    join(root, "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "dart-decimate"\nversion = "${lock}"\n`,
  );
  return root;
}

/** @param {string} root @param {string} cargo @param {string} npm */
function writeVersions(root, cargo, npm) {
  writeFileSync(
    join(root, "Cargo.toml"),
    `[package]\nname = "dart-decimate"\nversion = "${cargo}"\n`,
  );
  writeFileSync(
    join(root, "package.json"),
    JSON.stringify({ name: "dart-decimate", version: npm }),
  );
}

/** @param {string} cwd @param {NodeJS.ProcessEnv} [env] */
function checkSync(cwd, env = {}) {
  return runNode([versionSync], { cwd, env: { ...cleanEnv, ...env } });
}

test("version sync accepts matching manifests and lockfile", async () => {
  const result = await checkSync(project({ cargo: "1.2.3", npm: "1.2.3" }));

  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "version ok: 1.2.3");
});

test("version sync rejects manifests that disagree", async () => {
  const result = await checkSync(project({ cargo: "1.2.3", npm: "1.2.4" }));

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /version mismatch: Cargo.toml=1.2.3 package.json=1.2.4/,
  );
});

test("version sync rejects a stale Cargo.lock", async () => {
  const root = project({ cargo: "1.2.3", npm: "1.2.3", lock: "1.2.2" });

  const result = await checkSync(root);

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /version mismatch: Cargo.lock=1.2.2 expected=1.2.3/,
  );
});

test("version sync rejects a release tag for another version", async () => {
  const root = project({ cargo: "1.2.3", npm: "1.2.3" });

  const result = await checkSync(root, {
    GITHUB_REF_NAME: "v1.2.4",
    GITHUB_REF_TYPE: "tag",
  });

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /tag v1.2.4 does not match package version 1.2.3/,
  );
});

/** @param {string} root @param {string[]} args */
function git(root, args) {
  const result = spawnSync(
    "git",
    ["-c", "user.name=Test", "-c", "user.email=test@example.com", ...args],
    { cwd: root, encoding: "utf8", env: cleanEnv },
  );
  assert.equal(result.status, 0, result.stderr);
}

/** @param {string} base @param {string} next @param {string} [nextNpm] */
async function bump(base, next, nextNpm = next) {
  const root = mkdtempSync(join(tmpdir(), "dart-decimate-version-bump-"));
  git(root, ["init", "-q"]);
  writeVersions(root, base, base);
  git(root, ["add", "."]);
  git(root, ["commit", "-q", "-m", "base"]);
  writeVersions(root, next, nextNpm);
  return runNode([versionBump, "HEAD"], { cwd: root });
}

for (const [base, next] of [
  ["0.0.49", "0.0.50"],
  ["0.9.9", "1.0.0"],
  ["1.0.0-rc.1", "1.0.0"],
  ["1.0.0-rc.2", "1.0.0-rc.10"],
  ["1.0.0-alpha", "1.0.0-beta"],
  ["1.0.0-alpha", "1.0.0-alpha.1"],
]) {
  test(`version bump accepts ${base} -> ${next}`, async () => {
    const result = await bump(base, next);

    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, new RegExp(`Cargo.toml ${base} -> ${next}`));
  });
}

for (const [base, next] of [
  ["0.0.50", "0.0.50"],
  ["0.0.50", "0.0.49"],
  ["1.0.0", "1.0.0-rc.1"],
  ["1.0.0-rc.10", "1.0.0-rc.9"],
  ["1.0.0-alpha.1", "1.0.0-alpha"],
  ["1.0.0-beta", "1.0.0-1"],
]) {
  test(`version bump rejects ${base} -> ${next}`, async () => {
    const result = await bump(base, next);

    assert.equal(result.status, 1);
    assert.match(
      result.stderr,
      new RegExp(`Cargo.toml version must be bumped: ${base} -> ${next}`),
    );
  });
}

test("version bump rejects manifests that disagree", async () => {
  const result = await bump("1.0.0", "1.0.1", "1.0.2");

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /version mismatch: Cargo.toml=1.0.1 package.json=1.0.2/,
  );
});

test("version bump rejects an invalid version", async () => {
  const result = await bump("1.0.0", "1.0");

  assert.equal(result.status, 1);
  assert.match(result.stderr, /Cargo.toml has invalid semver: 1.0/);
});

test("version bump reports a base it cannot read", async () => {
  const root = mkdtempSync(join(tmpdir(), "dart-decimate-version-bump-"));
  git(root, ["init", "-q"]);
  writeVersions(root, "1.0.0", "1.0.0");

  const result = await runNode([versionBump, "missing-base"], { cwd: root });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /could not read Cargo.toml from missing-base/);
});

/** @param {import("node:http").RequestListener} registry */
function checkRelease(registry) {
  const cache = mkdtempSync(join(tmpdir(), "dart-decimate-npm-cache-"));
  return withServer(registry, (baseUrl) =>
    runNode([releaseVersion], {
      env: {
        ...cleanEnv,
        npm_config_cache: cache,
        npm_config_fetch_retries: "0",
        npm_config_registry: `${baseUrl}/`,
      },
    }),
  );
}

test("release check accepts a version the registry does not have", async () => {
  const result = await checkRelease((_request, response) => {
    response.writeHead(404, { "content-type": "application/json" });
    response.end(JSON.stringify({ error: "Not found" }));
  });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /release version ok: .* is not published/);
});

test("release check rejects a version already on the registry", async () => {
  const result = await checkRelease((_request, response) => {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(
      JSON.stringify({
        name: "dart-decimate",
        "dist-tags": { latest: packageVersion },
        versions: {
          [packageVersion]: {
            name: "dart-decimate",
            version: packageVersion,
            dist: { tarball: "http://127.0.0.1/dart-decimate.tgz" },
          },
        },
      }),
    );
  });

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    new RegExp(`dart-decimate@${packageVersion} is already published`),
  );
});

test("release check fails when the registry cannot answer", async () => {
  const result = await checkRelease((_request, response) => {
    response.writeHead(500);
    response.end();
  });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /could not verify npm version/);
});
