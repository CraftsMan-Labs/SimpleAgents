/**
 * Event names emitted by the Rust YAML workflow runner (stream callbacks).
 * Source of truth: crates/simple-agents-workflow/src/yaml_runner/ (execute.rs,
 * client_executor.rs, node_execution.rs).
 * The runner may add new types without a semver bump; treat as open-ended.
 */
export type WorkflowRunnerEventType =
  | 'workflow_started'
  | 'workflow_completed'
  | 'node_started'
  | 'node_completed'
  | 'resolved_llm_input'
  | 'node_stream_delta'
  | 'node_stream_snapshot'
  | 'node_stream_thinking_delta'
  | 'node_stream_output_delta'
  | 'node_tool_call_requested'
  | 'node_tool_call_completed'
  | 'node_tool_call_failed'
  | 'node_tool_roundtrip_completed'
  | 'node_healed'
  | (string & {})

export interface WorkflowRunnerEvent {
  event_type: WorkflowRunnerEventType
  node_id?: string
  step_id?: string
  node_kind?: string
  streamable?: boolean
  message?: string
  delta?: string
  snapshot?: unknown
  token_kind?: string
  is_terminal_node_token?: boolean
  elapsed_ms?: number
  metadata?: Record<string, unknown>
}

/**
 * Parse `eventJson` from the `executeWorkflowYamlStream` callback (same shape as Rust serde JSON).
 */
export function parseWorkflowEvent(eventJson: string): WorkflowRunnerEvent

/**
 * Ready-made `onEvent` callback for {@link Client.streamWorkflow} that prints
 * streamed tokens to stdout and silences lifecycle events.
 *
 * Usage:
 * ```ts
 * import { defaultOnEvent } from 'simple-agents-node/workflow_event';
 * await client.streamWorkflow(path, input, defaultOnEvent);
 * ```
 */
export function defaultOnEvent(err: unknown, eventJson: string): void
