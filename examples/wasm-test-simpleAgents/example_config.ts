import type { ClientConfig } from "../../bindings/wasm/simple-agents-wasm/index.js";

export function clientConfig(
  apiKey: string,
  baseUrl: string | undefined,
): ClientConfig {
  return {
    apiKey,
    baseUrl,
    fetchImpl: globalThis.fetch,
    timeoutSeconds: process.env.WORKFLOW_TIMEOUT_SECONDS
      ? Number(process.env.WORKFLOW_TIMEOUT_SECONDS)
      : undefined,
    retryAttempts: process.env.WORKFLOW_RETRY_ATTEMPTS
      ? Number(process.env.WORKFLOW_RETRY_ATTEMPTS)
      : undefined,
    retryStrategy: process.env.WORKFLOW_RETRY_STRATEGY as
      | ClientConfig["retryStrategy"]
      | undefined,
  };
}
