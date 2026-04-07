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

function getSellerNameFromPayload(payload: unknown): string {
  if (payload === null || typeof payload !== "object") {
    return "unknown";
  }
  const raw = (payload as Record<string, unknown>).company_name;
  if (raw === undefined || raw === null) {
    return "unknown";
  }
  const key = String(raw).trim().toLowerCase();
  if (!key) {
    return "unknown";
  }
  return stakeholderMap[key] ?? "unknown";
}

/** Pass this as `opts.customWorker` / `customWorkerDispatch`. */
export function customWorkerDispatch(req: CustomWorkerRequest): string {
  if (req.handler === "get_seller_name") {
    return getSellerNameFromPayload(req.payload);
  }
  throw new Error(`unknown custom_worker handler: ${req.handler}`);
}