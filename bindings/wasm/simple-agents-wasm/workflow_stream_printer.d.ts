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

/**
 * Ready-made `onEvent` callback that prints streamed tokens inline and silences
 * lifecycle events. A simpler alternative to {@link createWorkflowStreamPrinter}
 * (no step headers, no state).
 *
 * Usage:
 * ```ts
 * import { defaultOnEvent } from 'simple-agents-wasm/workflow_stream_printer';
 * await client.runWorkflow(yaml, input, { onEvent: defaultOnEvent });
 * ```
 */
export declare function defaultOnEvent(event: Record<string, unknown>): void;
