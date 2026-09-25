import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { test } from "node:test";

// Budget and samples are recorded in features/install-hard-eng/PLAN.md.
const MEDIAN_BUDGET_MS = 250;
const SAMPLES = 15;
const binary = join(
  "target",
  "release",
  process.platform === "win32" ? "dart-decimate.exe" : "dart-decimate",
);
const fixture = join("tests", "fixtures", "install-parity");

test("check scans the Flutter fixture app within the latency budget", () => {
  runCheck();
  const durations = Array.from({ length: SAMPLES }, () => {
    const started = performance.now();
    runCheck();
    return performance.now() - started;
  }).sort((left, right) => left - right);
  const median = durations[Math.floor(SAMPLES / 2)];

  console.log(
    `check median ${median.toFixed(1)}ms, max ${durations[SAMPLES - 1].toFixed(1)}ms over ${SAMPLES} runs`,
  );
  assert.ok(
    median <= MEDIAN_BUDGET_MS,
    `median ${median.toFixed(1)}ms exceeds ${MEDIAN_BUDGET_MS}ms`,
  );
});

function runCheck() {
  const result = spawnSync(
    binary,
    ["check", fixture, "--format", "json", "--quiet"],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  assert.equal(report.verdict, "pass");
  assert.equal(report.summary.files, 7);
}
