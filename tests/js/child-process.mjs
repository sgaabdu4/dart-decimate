import { spawn } from "node:child_process";
import { createServer } from "node:http";

/** @typedef {{ status: number | null, stdout: string, stderr: string }} NodeResult */

/** Git variables a hook exports; tests drop them so fixture repositories stay isolated. */
export const cleanEnv = Object.fromEntries(
  Object.entries(process.env).filter(([name]) => !name.startsWith("GIT_")),
);

/** @param {string[]} args @param {import("node:child_process").SpawnOptionsWithoutStdio} [options] @returns {Promise<NodeResult>} */
export function runNode(args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, {
      env: cleanEnv,
      ...options,
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (status) => resolve({ status, stdout, stderr }));
  });
}

/** @template T @param {import("node:http").RequestListener} handler @param {(baseUrl: string) => Promise<T>} use @returns {Promise<T>} */
export async function withServer(handler, use) {
  const server = createServer(handler);
  await new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => resolve(undefined)),
  );
  const { port } = /** @type {import("node:net").AddressInfo} */ (
    server.address()
  );
  try {
    return await use(`http://127.0.0.1:${port}`);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(() => resolve(undefined)));
  }
}
