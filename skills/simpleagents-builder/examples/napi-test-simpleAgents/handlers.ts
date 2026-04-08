/**
 * Custom worker handlers for `test.yaml` (`custom_worker.handler` values).
 *
 * Parity with `examples/python-test-simpleAgents/handlers.py`: the Rust runner
 * loads `handlers.py` next to the workflow; in Node you pass the dispatch
 * function via `customWorker` on `Client.run` / `Client.stream`.
 */

const stakeholderMap: Record<string, string> = {
  google: "Sundar Pichai",
  microsoft: "Satya Nadella",
  apple: "Tim Cook",
  amazon: "Andy Jassy",
};

export type CustomWorkerRequest = {
  handler: string;
  handlerFile?: string;
  payload: unknown;
  context: unknown;
};

/**
 * Resolve seller display name from invoice context (`config.payload` in YAML).
 * Same behavior as Python `get_seller_name(context, payload)`.
 */
export function getSellerName(_context: unknown, payload: unknown): string {
  let companyName: string | undefined;
  if (payload !== null && typeof payload === "object") {
    const raw = (payload as Record<string, unknown>).company_name;
    companyName =
      raw === undefined || raw === null ? undefined : String(raw).trim().toLowerCase();
  }
  if (!companyName) {
    return "unknown";
  }
  return stakeholderMap[companyName] ?? "unknown";
}

/** Pass as `opts.customWorker` / `customWorkerDispatch` on workflow calls. */
export function customWorkerDispatch(req: CustomWorkerRequest): string {
  if (req.handler === "get_seller_name") {
    return getSellerName(req.context, req.payload);
  }
  throw new Error(`unknown custom_worker handler: ${req.handler}`);
}
