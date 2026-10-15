import { createServer } from "node:http";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { format } from "node:util";

/** @typedef {{ status: number, stdout: string, stderr: string }} NodeResult */

/** Git variables a hook exports; tests drop them so fixture repositories stay isolated. */
export const cleanEnv = Object.fromEntries(
  Object.entries(process.env).filter(([name]) => !name.startsWith("GIT_")),
);

class ExitSignal extends Error {}

const require = createRequire(import.meta.url);

/** Bun's coverage only sees code loaded in the test process, so scripts run in-process with their own cwd, env and argv. @param {string[]} args @param {{ cwd?: string, env?: NodeJS.ProcessEnv }} [options] @returns {Promise<NodeResult>} */
export async function runNode(args, options = {}) {
  const [script, ...argv] = args;
  const { cwd = process.cwd(), env = cleanEnv } = options;
  const saved = {
    cwd: process.cwd(),
    env: { ...process.env },
    argv: process.argv,
    exit: process.exit,
    console: { ...console },
    stdoutWrite: process.stdout.write,
    stderrWrite: process.stderr.write,
  };
  let stdout = "";
  let stderr = "";
  /** @type {number | undefined} */
  let status;
  const exited = Promise.withResolvers();
  /** @param {"stdout" | "stderr"} stream @param {string} text */
  const capture = (stream, text) => {
    if (status !== undefined) {
      return;
    }
    if (stream === "stdout") {
      stdout += text;
    } else {
      stderr += text;
    }
  };
  /** @param {"stdout" | "stderr"} stream */
  const consoleWriter =
    (stream) =>
    /** @param {unknown[]} values */
    (...values) => {
      capture(stream, `${format(...values)}\n`);
    };
  /** @param {"stdout" | "stderr"} stream */
  const streamWriter =
    (stream) =>
    /** @param {unknown} chunk */
    (chunk) => {
      capture(stream, String(chunk));
      return true;
    };
  try {
    process.chdir(cwd);
    for (const name of Object.keys(process.env)) {
      delete process.env[name];
    }
    Object.assign(process.env, env);
    process.argv = [process.execPath, script, ...argv];
    Reflect.set(process, "exit", (code = 0) => {
      if (status !== undefined) {
        return;
      }
      status = Number(code);
      exited.resolve(undefined);
      throw new ExitSignal();
    });
    Reflect.set(console, "log", consoleWriter("stdout"));
    Reflect.set(console, "info", consoleWriter("stdout"));
    Reflect.set(console, "warn", consoleWriter("stderr"));
    Reflect.set(console, "error", consoleWriter("stderr"));
    Reflect.set(process.stdout, "write", streamWriter("stdout"));
    Reflect.set(process.stderr, "write", streamWriter("stderr"));
    delete require.cache[script];
    const loaded = import(pathToFileURL(script).href).then(
      () => undefined,
      (error) => {
        if (!(error instanceof ExitSignal)) {
          status ??= 1;
          stderr += `${error instanceof Error ? error.stack : error}\n`;
        }
      },
    );
    if (script.endsWith(".js")) {
      await Promise.race([exited.promise, loaded.then(() => exited.promise)]);
    } else {
      await loaded;
    }
    await new Promise((resolve) => setImmediate(resolve));
  } finally {
    process.chdir(saved.cwd);
    for (const name of Object.keys(process.env)) {
      delete process.env[name];
    }
    Object.assign(process.env, saved.env);
    process.argv = saved.argv;
    Reflect.set(process, "exit", saved.exit);
    Object.assign(console, saved.console);
    Reflect.set(process.stdout, "write", saved.stdoutWrite);
    Reflect.set(process.stderr, "write", saved.stderrWrite);
  }
  return { status: status ?? 0, stdout, stderr };
}

/** @template T @param {import("node:http").RequestListener} handler @param {(baseUrl: string) => Promise<T>} use @returns {Promise<T>} */
export async function withServer(handler, use) {
  const server = createServer(handler);
  await new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => resolve(undefined)),
  );
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("test server is not listening on a TCP port");
  }
  try {
    return await use(`http://127.0.0.1:${address.port}`);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(() => resolve(undefined)));
  }
}
