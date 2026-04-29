#!/usr/bin/env node
/**
 * Run `node --test …` with variables from `<monorepo-root>/.env` when present.
 * Avoids `--env-file-if-exists` (Node 22.9+) so Node 18+ matches package engines.
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootEnv = join(__dirname, "..", "..", "..", ".env");

if (existsSync(rootEnv)) {
  const { config } = await import("dotenv");
  config({ path: rootEnv, override: false });
}

const testFiles = process.argv.slice(2);
if (testFiles.length === 0) {
  console.error("usage: node scripts/node-test-with-env.mjs <test-file> [...]");
  process.exit(1);
}

const result = spawnSync(
  process.execPath,
  ["--test", ...testFiles],
  { stdio: "inherit", env: process.env },
);

process.exit(result.status ?? 1);
