export type ReasoningEffort = "none" | "min" | "low" | "medium" | "high" | "xhigh" | "max" | number;

export interface CompleteOptions {
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  stream?: boolean;
  mode?: "standard" | "healed_json" | "schema";
  schema?: unknown;
  reasoningEffort?: ReasoningEffort;
}

/**
 * Simplified multimodal segment (normalized to OpenAI wire format before send).
 * You may also pass native OpenAI parts (e.g. `{ type: "image_url", image_url: { url } }`).
 */
export interface ContentPartInput {
  type: "text" | "image" | "audio" | "video";
  text?: string;
  /** Base64 payload without data: prefix */
  data?: string;
  /** MIME type, e.g. image/png (also accepts camelCase `mediaType`) */
  media_type?: string;
  mediaType?: string;
}

export interface MessageInput {
  role: "system" | "user" | "assistant" | "tool";
  content: string | ContentPartInput[];
  name?: string;
  toolCallId?: string;
  toolCalls?: Array<JsToolCall>;
}

export interface JsToolCallFunction {
  name: string;
  arguments: string;
}

export interface JsToolCall {
  id: string;
  toolType: string;
  function: JsToolCallFunction;
}

export interface ToolCallResultFunction {
  name: string;
  arguments: string;
}

export interface ToolCallResult {
  id: string;
  toolType: string;
  function: ToolCallResultFunction;
}

export interface CompletionUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

export interface HealingData {
  value?: unknown;
  flags: string[];
  confidence: number;
  error?: string;
}

export interface CompletionResult {
  id: string;
  model: string;
  role: string;
  content?: string;
  toolCalls?: Array<ToolCallResult>;
  finishReason?: string;
  usage: CompletionUsage;
  usageAvailable: boolean;
  latencyMs: number;
  raw?: string;
  healed?: HealingData;
  coerced?: HealingData;
}

export interface StreamChunk {
  id: string;
  model: string;
  content?: string;
  finishReason?: string;
  error?: string;
  raw?: string;
}

export interface StreamDelta {
  id: string;
  model: string;
  index: number;
  role?: string;
  content?: string;
  finishReason?: string;
  raw?: string;
}

export interface StreamErrorEvent {
  message: string;
}

export interface StreamEvent {
  eventType: "delta" | "error" | "done";
  delta?: StreamDelta;
  error?: StreamErrorEvent;
}

export interface ClientConfig {
  baseUrl?: string;
  apiKey: string;
  fetchImpl?: typeof fetch;
  headers?: Record<string, string>;
  timeoutSeconds?: number;
  retryAttempts?: number;
  retryStrategy?: "none" | "fixed" | "exponential";
}

/** Matches Rust `YamlWorkflowTelemetryConfig` JSON (snake_case). */
export interface WorkflowTelemetryConfig {
  enabled?: boolean;
  nerdstats?: boolean;
  sample_rate?: number;
  payload_mode?: "full_payload" | "redacted_payload";
  retention_days?: number;
  multi_tenant?: boolean;
  tool_trace_mode?: "full" | "redacted" | "off";
}

export interface WorkflowTraceContext {
  trace_id?: string;
  span_id?: string;
  parent_span_id?: string;
  traceparent?: string;
  tracestate?: string;
  baggage?: Record<string, string>;
}

export interface WorkflowTraceTenant {
  workspace_id?: string;
  user_id?: string;
  conversation_id?: string;
  request_id?: string;
  run_id?: string;
}

export interface WorkflowTraceConfig {
  context?: WorkflowTraceContext;
  tenant?: WorkflowTraceTenant;
}

/**
 * First argument to `workflow_options.functions` handlers for `custom_worker` nodes
 * (Rust wasm graph runner). Matches the JSON object built in Rust.
 */
export interface CustomWorkerArgs {
  handler?: string;
  handler_file?: string | null;
  handler_lookup_key?: string;
  nodeId?: string;
  payload?: unknown;
}

/**
 * Second argument to `custom_worker` handlers: live graph context (`input`, `nodes`, and
 * optional `globals` / `trace` when the runner provides them).
 */
export interface WorkflowGraphContext {
  input?: Record<string, unknown>;
  nodes?: Record<string, unknown>;
  globals?: Record<string, unknown>;
  trace?: Record<string, unknown>;
  [key: string]: unknown;
}

export interface WorkflowRunOptions {
  model?: string;
  telemetry?: WorkflowTelemetryConfig;
  trace?: WorkflowTraceConfig;
  onEvent?: (event: Record<string, unknown>) => void;
  functions?: Record<
    string,
    (
      args: CustomWorkerArgs,
      context: WorkflowGraphContext
    ) => unknown | Promise<unknown>
  >;
}

/** Common workflow `input` fields; extra keys are allowed for workflow-specific payloads. */
export interface WorkflowInputFields {
  email_text?: string;
  [key: string]: unknown;
}

export interface WorkflowRunEvent {
  stepId: string;
  stepType: string;
  status: "started" | "completed";
}

export interface WorkflowRunResult {
  status: "ok" | "completed" | "awaiting_human_input";
  context: Record<string, unknown>;
  output?: unknown;
  human_request?: Record<string, unknown>;
  events: WorkflowRunEvent[];
}

export interface WorkflowExecutionFlags {
  model?: string;
  healing?: boolean;
  workflow_streaming?: boolean;
  node_llm_streaming?: boolean;
  split_stream_deltas?: boolean;
  debug_stream_parse?: boolean;
}

export interface WorkflowExecutionRequest {
  workflow_yaml: string;
  messages?: MessageInput[];
  context?: Record<string, unknown>;
  media?: Record<string, unknown>;
  input?: WorkflowInputFields;
  resume?: Record<string, unknown>;
  human_response?: unknown;
  execution?: WorkflowExecutionFlags;
  workflow_options?: WorkflowRunOptions;
}

export declare class Client {
  constructor(provider: string, config: ClientConfig);
  complete(
    model: string,
    promptOrMessages: string | MessageInput[],
    options?: CompleteOptions
  ): Promise<CompletionResult>;
  stream(
    model: string,
    promptOrMessages: string | MessageInput[],
    onChunk: (chunk: StreamChunk) => void,
    options?: CompleteOptions
  ): Promise<CompletionResult>;
  streamEvents(
    model: string,
    promptOrMessages: string | MessageInput[],
    onEvent: (event: StreamEvent) => void,
    options?: CompleteOptions
  ): Promise<CompletionResult>;
  runWorkflowYamlString(
    yamlText: string,
    workflowInput: Record<string, unknown>,
    workflowOptions?: WorkflowRunOptions
  ): Promise<WorkflowRunResult>;
  runWorkflowYaml(
    workflowPath: string,
    workflowInput: Record<string, unknown>
  ): Promise<never>;
  run(request: WorkflowExecutionRequest): Promise<WorkflowRunResult>;
  runAsync(request: WorkflowExecutionRequest): Promise<WorkflowRunResult>;
  streamWorkflow(
    request: WorkflowExecutionRequest,
    onEvent?: (event: Record<string, unknown>) => void
  ): Promise<WorkflowRunResult>;
}

export declare function hasRustBackend(): Promise<boolean>;
