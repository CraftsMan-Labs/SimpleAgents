/**
 * Shared filesystem layout for `examples/napi-test-simpleAgents/`.
 *
 * Mirrors `examples/python-test-simpleAgents/example_paths.py`.
 */
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));

export const PACKAGE_ROOT = __dirname;

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
