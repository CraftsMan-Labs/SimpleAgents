/**
 * Shared filesystem layout for `examples/napi-test-simpleAgents/`.
 *
 * Mirrors `examples/python-test-simpleAgents/example_paths.py`.
 */
import { config as loadEnv } from "dotenv";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const PACKAGE_ROOT = __dirname;

/** `examples/` (parent of this package). */
export const EXAMPLES_ROOT = join(PACKAGE_ROOT, "..");

/** Monorepo root (`SimpleAgents/`, parent of `examples/`). */
export const REPO_ROOT = join(PACKAGE_ROOT, "..", "..");

/**
 * Load `.env` from the monorepo root, then `examples/`, then this package.
 * Package-level keys override (useful for local overrides).
 */
export function loadNapiExampleEnv(): void {
  loadEnv({ path: join(REPO_ROOT, ".env") });
  loadEnv({ path: join(EXAMPLES_ROOT, ".env") });
  loadEnv({ path: join(PACKAGE_ROOT, ".env"), override: true });
}

export function pathToWorkflow(...parts: string[]): string {
  return join(PACKAGE_ROOT, "workflows", ...parts);
}

export function pathToEvalSuite(...parts: string[]): string {
  return join(PACKAGE_ROOT, "evals", ...parts);
}

/** Local assets (`assets/` inside this package). */
export function pathToAsset(...parts: string[]): string {
  return join(PACKAGE_ROOT, "assets", ...parts);
}

/** Invoice image etc. shared with the Python sibling example. */
export function pathToPythonExamplesAsset(...parts: string[]): string {
  return join(PACKAGE_ROOT, "..", "python-test-simpleAgents", "assets", ...parts);
}
