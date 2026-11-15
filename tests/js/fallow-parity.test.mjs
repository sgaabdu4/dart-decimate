import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";

const WRAPPER = "npm/bin/dart-decimate.js";

/** @param {Record<string, string>} files @param {string[]} args @returns {{ status: number | null, report: any }} */
function run(files, args) {
  const root = mkdtempSync(join(tmpdir(), "dd-parity-"));
  try {
    for (const [path, source] of Object.entries(files)) {
      mkdirSync(dirname(join(root, path)), { recursive: true });
      writeFileSync(join(root, path), source);
    }
    const result = spawnSync(
      process.execPath,
      [WRAPPER, ...args.map((arg) => arg.replace("$ROOT", root))],
      {
        cwd: process.cwd(),
        encoding: "utf8",
        env: { ...process.env, DART_DECIMATE_SKIP_BUILD: "1" },
      },
    );
    return { status: result.status, report: JSON.parse(result.stdout) };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

/** @param {Record<string, string>} files */
function strictCheck(files) {
  return run(files, [
    "check",
    "$ROOT",
    "--threshold",
    "0",
    "--strict",
    "--format",
    "json",
  ]);
}

/** @param {any} report @returns {string[]} */
function ruleIds(report) {
  return JSON.stringify(report).match(/dart-decimate\/[a-z-]+/g) ?? [];
}

const OVERRIDE_LOCK =
  'packages:\n  helper:\n    dependency: "direct main"\n    description:\n      path: helper\n      relative: true\n    source: path\n    version: "1.0.0"\n  spare:\n    dependency: "direct overridden"\n    description:\n      name: spare\n    source: hosted\n    version: "1.0.0"\n';

const OVERRIDE_GRAPH =
  '{"roots":["app"],"packages":[{"name":"app","dependencies":["helper"],"devDependencies":[]},{"name":"helper","dependencies":[]},{"name":"spare","dependencies":[]}],"configVersion":1}';

const CRAP_LCOV =
  "SF:lib/main.dart\nDA:2,0\nDA:3,0\nDA:4,0\nDA:5,0\nend_of_record\n";

const CRAP_SOURCE =
  "void uncovered(List<int> items) {\n  if (items.isEmpty) return;\n  for (final item in items) {\n    if (item.isEven) return;\n  }\n}\n";

/** @param {number} limit */
function crapRun(limit) {
  return run(
    { "lib/main.dart": CRAP_SOURCE, "coverage/lcov.info": CRAP_LCOV },
    [
      "health",
      "$ROOT",
      "--format",
      "json",
      "--coverage",
      "coverage/lcov.info",
      "--max-crap",
      String(limit),
    ],
  );
}

test("an unused public method of a plain class fails the strict check", () => {
  const { status, report } = strictCheck({
    "pubspec.yaml": "name: app\n",
    "lib/main.dart":
      "import 'src/cleaner.dart';\n\nvoid main() {\n  print(Cleaner().clean(' x '));\n}\n",
    "lib/src/cleaner.dart":
      "class Cleaner {\n  String clean(String text) => text.trim();\n\n  int spare() => 1;\n}\n",
  });

  assert.equal(status, 1);
  assert.ok(ruleIds(report).includes("dart-decimate/unused-class-member"));
});

test("a class member that another file uses keeps the strict check green", () => {
  const { status } = strictCheck({
    "pubspec.yaml": "name: app\n",
    "lib/main.dart":
      "import 'src/cleaner.dart';\n\nvoid main() {\n  print(Cleaner().clean(' x '));\n}\n",
    "lib/src/cleaner.dart":
      "class Cleaner {\n  String clean(String text) => text.trim();\n}\n",
  });

  assert.equal(status, 0);
});

test("one public name declared in two library files fails the strict check", () => {
  const { status, report } = strictCheck({
    "pubspec.yaml": "name: package\n",
    "lib/package.dart":
      "import 'src/a.dart';\nimport 'src/b.dart';\n\nvoid run() {\n  first();\n  second();\n}\n",
    "lib/src/a.dart": "class Label {}\n\nvoid first() {}\n",
    "lib/src/b.dart": "class Label {}\n\nvoid second() {}\n",
  });

  assert.equal(status, 1);
  assert.ok(ruleIds(report).includes("dart-decimate/duplicate-export"));
});

test("same-name files that no library imports together keep the strict check green", () => {
  const { status } = strictCheck({
    "pubspec.yaml": "name: package\n",
    "lib/package.dart":
      "import 'src/x.dart';\nimport 'src/y.dart';\n\nvoid run() {\n  xmake();\n  ymake();\n}\n",
    "lib/src/x.dart": "import 'a.dart';\n\nLabel xmake() => Label();\n",
    "lib/src/y.dart": "import 'b.dart';\n\nLabel ymake() => Label();\n",
    "lib/src/a.dart": "class Label {}\n",
    "lib/src/b.dart": "class Label {}\n",
  });

  assert.equal(status, 0);
});

test("platform alternatives that share a name keep the strict check green", () => {
  const { status } = strictCheck({
    "pubspec.yaml": "name: package\n",
    "lib/package.dart":
      "import 'src/stub.dart' if (dart.library.io) 'src/io.dart';\n\nvoid run() {\n  createClient();\n}\n",
    "lib/src/stub.dart": "Object createClient() => 0;\n",
    "lib/src/io.dart": "Object createClient() => 1;\n",
  });

  assert.equal(status, 0);
});

test("an override that the dependency graph never reaches fails the strict check", () => {
  const { status, report } = strictCheck({
    "pubspec.yaml":
      "name: app\ndependencies:\n  helper:\n    path: helper\ndependency_overrides:\n  spare: ^1.0.0\n",
    "pubspec.lock": OVERRIDE_LOCK,
    "helper/pubspec.yaml": "name: helper\n",
    "helper/lib/helper.dart": "void help() {}\n",
    "lib/main.dart": "import 'package:helper/helper.dart';\nvoid main() {}\n",
    ".dart_tool/package_graph.json": OVERRIDE_GRAPH,
  });

  assert.equal(status, 1);
  assert.ok(
    ruleIds(report).includes("dart-decimate/unused-dependency-override"),
  );
});

test("an override stays when the dependency graph file is missing", () => {
  const { status } = strictCheck({
    "pubspec.yaml":
      "name: app\ndependencies:\n  helper:\n    path: helper\ndependency_overrides:\n  spare: ^1.0.0\n",
    "pubspec.lock": OVERRIDE_LOCK,
    "helper/pubspec.yaml": "name: helper\n",
    "helper/lib/helper.dart": "void help() {}\n",
    "lib/main.dart": "import 'package:helper/helper.dart';\nvoid main() {}\n",
  });

  assert.equal(status, 0);
});

test("a blank string override fails the strict check as misconfigured", () => {
  const { status, report } = strictCheck({
    "pubspec.yaml":
      'name: app\ndependencies:\n  helper:\n    path: helper\ndependency_overrides:\n  helper: ""\n',
    "helper/pubspec.yaml": "name: helper\n",
    "lib/main.dart": "import 'package:helper/helper.dart';\nvoid main() {}\n",
    "helper/lib/helper.dart": "void help() {}\n",
  });

  assert.equal(status, 1);
  assert.ok(
    ruleIds(report).includes("dart-decimate/misconfigured-dependency-override"),
  );
});

test("two workspace packages that depend on each other fail the strict check", () => {
  const { status, report } = strictCheck({
    "pubspec.yaml":
      "name: root\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - packages/a\n  - packages/b\n",
    "packages/a/pubspec.yaml":
      "name: a\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  b:\n    path: ../b\n",
    "packages/b/pubspec.yaml":
      "name: b\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  a:\n    path: ../a\n",
    "packages/a/lib/a.dart":
      "import 'package:b/b.dart';\n\nString fa() => fb();\n",
    "packages/b/lib/b.dart":
      "import 'package:a/other.dart';\n\nString fb() => other();\n",
    "packages/a/lib/other.dart": "String other() => 'o';\n",
    "packages/a/bin/main.dart":
      "import 'package:a/a.dart';\n\nvoid main() {\n  print(fa());\n}\n",
  });

  assert.equal(status, 1);
  assert.ok(ruleIds(report).includes("dart-decimate/package-cycle"));
});

test("workspace packages with one-way dependencies keep the strict check green", () => {
  const { status } = strictCheck({
    "pubspec.yaml":
      "name: root\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - packages/a\n  - packages/b\n",
    "packages/a/pubspec.yaml":
      "name: a\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  b:\n    path: ../b\n",
    "packages/b/pubspec.yaml":
      "name: b\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\n",
    "packages/a/lib/a.dart":
      "import 'package:b/b.dart';\n\nString fa() => fb();\n",
    "packages/b/lib/b.dart": "String fb() => 'b';\n",
    "packages/a/bin/main.dart":
      "import 'package:a/a.dart';\n\nvoid main() {\n  print(fa());\n}\n",
  });

  assert.equal(status, 0);
});

test("a CRAP score equal to the limit is reported", () => {
  const atLimit = crapRun(20);
  const aboveLimit = crapRun(21);

  assert.equal(atLimit.status, 1);
  assert.ok(ruleIds(atLimit.report).includes("dart-decimate/high-crap-score"));
  assert.equal(aboveLimit.status, 0);
});
