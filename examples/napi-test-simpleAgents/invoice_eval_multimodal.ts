/**
 * Shared multimodal invoice eval payloads (Jaeger demos + invoice eval JSONL).
 *
 * Matches `examples/python-test-simpleAgents/invoice_eval_multimodal.py`: Chat-style
 * `text` + `image_url` parts so `parse_messages_value` accepts dataset rows unchanged.
 */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

export const INVOICE_USER_TEXT_FOR_EVAL = `Invoice image. Classify and route this per workflow.

Invoice issuer/vendor: Google
Invoice type: cloud services invoice
Amount due: $50,000
Please classify this as the invoice workflow would classify the attached image.`;

const CASE_ID_TERMINAL = "google-invoice-terminal-node";
const CASE_ID_NODE = "google-invoice-node-paths";

const EXPECTED_TERMINAL: Record<string, unknown> = {
  terminal_node: "finalize_invoice_classification",
};

const EXPECTED_NODE: Record<string, unknown> = {
  terminal_node: "finalize_invoice_classification",
  trace: [
    "detect_email_domain",
    "route_email_domain",
    "detect_finance_subtype",
    "route_finance_subtype",
    "extract_invoice_company_name",
    "lookup_invoice_stakeholder",
    "finalize_invoice_classification",
  ],
  outputs: {
    detect_email_domain: { output: { domain: "finance" } },
    detect_finance_subtype: { output: { finance_subtype: "invoice" } },
    extract_invoice_company_name: { output: { company_name: "Google" } },
    lookup_invoice_stakeholder: { output: "Sundar Pichai" },
    finalize_invoice_classification: {
      output: {
        top_level_category: "finance",
        subtype: "invoice",
        label: "finance/invoice",
        company_name: "Google",
        stakeholder_name: "Sundar Pichai",
      },
    },
  },
};

export function multimodalInvoiceContentParts(imageB64: string): unknown[] {
  const dataUrl = `data:image/jpeg;base64,${imageB64}`;
  return [
    { type: "text", text: INVOICE_USER_TEXT_FOR_EVAL },
    { type: "image_url", image_url: { url: dataUrl } },
  ];
}

export function evalInputJson(imageB64: string): Record<string, unknown> {
  return {
    messages: [
      {
        role: "user",
        content: multimodalInvoiceContentParts(imageB64),
      },
    ],
  };
}

/**
 * Writes `invoice-image-{terminal,node}-eval.dataset.jsonl` under `invoiceDir/generated/`.
 */
export function writeInvoiceEvalGeneratedDatasets(invoiceDir: string, imagePath: string): void {
  mkdirSync(invoiceDir, { recursive: true });
  const generated = join(invoiceDir, "generated");
  mkdirSync(generated, { recursive: true });

  const imageB64 = readFileSync(imagePath).toString("base64");
  const inp = evalInputJson(imageB64);

  const terminalPath = join(generated, "invoice-image-terminal-eval.dataset.jsonl");
  writeFileSync(
    terminalPath,
    `${JSON.stringify({
      id: CASE_ID_TERMINAL,
      input: inp,
      expected_output: EXPECTED_TERMINAL,
    })}\n`,
    "utf-8",
  );

  const nodePath = join(generated, "invoice-image-node-eval.dataset.jsonl");
  writeFileSync(
    nodePath,
    `${JSON.stringify({
      id: CASE_ID_NODE,
      input: inp,
      expected_output: EXPECTED_NODE,
    })}\n`,
    "utf-8",
  );
}
