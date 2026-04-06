/**
 * @typedef {Object} WorkflowStreamPrinterOptions
 * @property {boolean} [splitThinking] When true, print `node_stream_thinking_delta` and
 *   `node_stream_output_delta`; otherwise print `node_stream_delta`.
 */

/**
 * Default `onEvent` handler for {@link Client.streamWorkflow} that prints stream tokens to stdout.
 * @param {WorkflowStreamPrinterOptions} [options]
 * @returns {(event: Record<string, unknown>) => void}
 */
function writeChunk(chunk) {
  if (typeof process !== "undefined" && process.stdout?.write) {
    process.stdout.write(chunk);
  } else if (typeof chunk === "string" && chunk.length > 0) {
    console.log(chunk);
  }
}

function writeLine() {
  if (typeof process !== "undefined" && process.stdout?.write) {
    process.stdout.write("\n");
  } else {
    console.log();
  }
}

export function createWorkflowStreamPrinter(options = {}) {
  const splitThinking = options.splitThinking === true;
  /** @type {{ currentNode: string | null, lineOpen: boolean, lastTokenLabel: string | null }} */
  const state = { currentNode: null, lineOpen: false, lastTokenLabel: null };

  return (event) => {
    const eventType = event?.event_type;
    if (typeof eventType !== "string") return;

    let isStream = eventType === "node_stream_delta";
    if (splitThinking) {
      isStream =
        eventType === "node_stream_thinking_delta" ||
        eventType === "node_stream_output_delta";
    }

    const delta = typeof event.delta === "string" ? event.delta : "";
    if (isStream && delta !== "") {
      let displayId =
        typeof event.node_id === "string"
          ? event.node_id
          : typeof event.step_id === "string"
            ? event.step_id
            : "?";

      if (state.currentNode !== displayId) {
        if (state.lineOpen) writeLine();
        writeChunk(`\nStep: ${displayId}\nStreaming: `);
        state.currentNode = displayId;
        state.lineOpen = true;
        state.lastTokenLabel = null;
      }

      if (splitThinking) {
        const parts = [];
        if (typeof event.token_kind === "string" && event.token_kind.trim()) {
          parts.push(event.token_kind.trim());
        }
        if (event.is_terminal_node_token === true) {
          parts.push("terminal");
        }
        const tokenLabel = parts.length ? `[${parts.join(" ")}] ` : "";
        if (tokenLabel && tokenLabel !== state.lastTokenLabel) {
          if (state.lineOpen) writeLine();
          writeChunk(`${tokenLabel}${displayId}: `);
          state.lastTokenLabel = tokenLabel;
          state.lineOpen = true;
        }
        writeChunk(delta);
      } else {
        writeChunk(delta);
      }
      return;
    }

    if (eventType === "workflow_started" || eventType === "workflow_completed") {
      return;
    }
  };
}

/**
 * Ready-made `onEvent` callback that prints streamed tokens inline and silences
 * lifecycle events. A simpler alternative to {@link createWorkflowStreamPrinter}
 * (no step headers, no state).
 *
 * Usage:
 * ```js
 * import { defaultOnEvent } from 'simple-agents-wasm/workflow_stream_printer';
 * await client.runWorkflow(yaml, input, { onEvent: defaultOnEvent });
 * ```
 *
 * @param {Record<string, unknown>} event
 */
export function defaultOnEvent(event) {
  const eventType = event?.event_type;
  if (typeof eventType !== "string") return;
  if (
    eventType === "node_stream_delta" ||
    eventType === "node_stream_thinking_delta" ||
    eventType === "node_stream_output_delta"
  ) {
    const delta = event.delta;
    if (typeof delta === "string") {
      writeChunk(delta);
    }
  }
}
