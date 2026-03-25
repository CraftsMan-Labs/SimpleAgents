export interface CompleteOptions {
  maxTokens?: number;
  temperature?: number;
  topP?: number;
  stream?: boolean;
  mode?: "standard" | "healed_json" | "schema";
  schema?: unknown;
}

export interface MessageInput {
  role: "system" | "user" | "assistant" | "tool";
  content: string;
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
}

export interface WorkflowRunOptions {
  telemetry?: Record<string, unknown>;
  trace?: Record<string, unknown>;
  functions?: Record<
    string,
    (
      args: Record<string, unknown>,
      context: Record<string, unknown>
    ) => unknown | Promise<unknown>
  >;
}

export interface WorkflowRunEvent {
  stepId: string;
  stepType: string;
  status: "started" | "completed";
}

export interface WorkflowRunResult {
  status: "ok";
  context: Record<string, unknown>;
  output?: unknown;
  events: WorkflowRunEvent[];
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
}

export declare function hasRustBackend(): Promise<boolean>;
