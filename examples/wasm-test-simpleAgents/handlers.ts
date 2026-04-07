/**
 * Custom worker handlers for the shared workflow YAML.
 *
 * In wasm mode, pass these via `workflow_options.functions`.
 */

import type {
  CustomWorkerArgs,
  WorkflowGraphContext,
} from "../../bindings/wasm/simple-agents-wasm/index.js";

const stakeholderMap: Record<string, string> = {
  google: "Sundar Pichai",
  microsoft: "Satya Nadella",
  apple: "Tim Cook",
  amazon: "Andy Jassy",
};

export function getSellerName(args: CustomWorkerArgs, _context: WorkflowGraphContext): string {
  let companyName: string | undefined;
  const payload = args?.payload;
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

export const workflowFunctions = {
  get_seller_name: getSellerName,
};
