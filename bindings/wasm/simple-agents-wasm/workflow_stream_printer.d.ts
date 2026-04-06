export interface WorkflowStreamPrinterOptions {
  /** When true, print thinking vs output deltas instead of merged `node_stream_delta`. */
  splitThinking?: boolean;
}

/**
 * Build a default `onEvent` callback for {@link Client.streamWorkflow} (Node-friendly streaming;
 * in browsers without `process.stdout`, falls back to `console.log` per chunk).
 */
export function createWorkflowStreamPrinter(
  options?: WorkflowStreamPrinterOptions,
): (event: Record<string, unknown>) => void;
