import { configError, runtimeError } from "./runtime/errors.js";
import {
  createStreamEventBridge,
} from "./runtime/stream.js";
import { loadRustModule } from "./runtime/rust-runtime.js";

function buildWorkflowInputFromExecutionRequest(request) {
  if (!request || typeof request !== "object") {
    throw configError("workflow request must be an object");
  }
  if (typeof request.workflow_yaml !== "string" || request.workflow_yaml.trim().length === 0) {
    throw configError("workflow_yaml must be a non-empty string");
  }
  if (!Array.isArray(request.messages) || request.messages.length === 0) {
    throw configError("messages must be a non-empty array");
  }

  const input = request.input && typeof request.input === "object" ? { ...request.input } : {};
  input.messages = request.messages;
  if (request.context && typeof request.context === "object") {
    input.context = request.context;
  }
  if (request.media && typeof request.media === "object") {
    input.media = request.media;
  }
  return input;
}

function buildWorkflowOptionsFromExecutionRequest(request, onEvent) {
  const execution = request.execution && typeof request.execution === "object"
    ? request.execution
    : {};
  const options = request.workflow_options && typeof request.workflow_options === "object"
    ? { ...request.workflow_options }
    : {};
  if (typeof execution.model === "string" && execution.model.trim().length > 0) {
    options.model = execution.model;
  }
  if (typeof onEvent === "function") {
    options.onEvent = onEvent;
  }
  return options;
}

function assertWorkflowResultShape(result) {
  if (result === null || typeof result !== "object") {
    throw runtimeError(
      "workflow result contract mismatch: expected an object with workflow_id and outputs"
    );
  }

  if (!("workflow_id" in result) || !("outputs" in result)) {
    throw runtimeError(
      "workflow result contract mismatch: expected keys 'workflow_id' and 'outputs'"
    );
  }

  return result;
}

function normalizeWorkflowResult(result) {
  if (result === null || typeof result !== "object") {
    return result;
  }
  if ("workflow_id" in result && "outputs" in result) {
    return result;
  }
  if (!("context" in result) || !result.context || typeof result.context !== "object") {
    return result;
  }

  const context = result.context;
  const nodeOutputs =
    context && typeof context === "object" && context.nodes && typeof context.nodes === "object"
      ? context.nodes
      : context;
  const trace = Array.isArray(result.events)
    ? result.events
      .filter((event) => event && event.status === "completed" && typeof event.stepId === "string")
      .map((event) => event.stepId)
    : [];
  const terminalNode = trace.at(-1) ?? "";

  return {
    workflow_id: typeof result.workflow_id === "string" ? result.workflow_id : "wasm_workflow",
    entry_node: typeof result.entry_node === "string" ? result.entry_node : trace[0] ?? "",
    email_text: typeof context?.input?.email_text === "string" ? context.input.email_text : "",
    trace,
    outputs: nodeOutputs,
    context,
    terminal_node: typeof result.terminal_node === "string" ? result.terminal_node : terminalNode,
    terminal_output: result.output,
    events: Array.isArray(result.events) ? result.events : [],
    status: typeof result.status === "string" ? result.status : "ok"
  };
}

export class Client {
  constructor(provider, config) {
    if (provider !== "openai" && provider !== "openrouter") {
      throw configError("provider must be 'openai' or 'openrouter' in wasm mode");
    }
    if (!config || typeof config !== "object") {
      throw configError("config object is required");
    }
    if (typeof config.apiKey !== "string" || config.apiKey.trim() === "") {
      throw configError("config.apiKey is required");
    }

    this.provider = provider;
    this.config = config;
    this.rustClient = null;
    this.readyPromise = null;
    this.fetchOverrideQueue = Promise.resolve();
  }

  async ensureBackend() {
    if (this.rustClient) {
      return this.rustClient;
    }
    if (this.readyPromise) {
      return this.readyPromise;
    }

    this.readyPromise = (async () => {
      const moduleValue = await loadRustModule();
      if (!moduleValue || typeof moduleValue.WasmClient !== "function") {
        throw runtimeError("Rust WASM backend is unavailable");
      }
      return new moduleValue.WasmClient(this.provider, {
        apiKey: this.config.apiKey,
        baseUrl: this.config.baseUrl,
        headers: this.config.headers
      });
    })();

    try {
      this.rustClient = await this.readyPromise;
      return this.rustClient;
    } catch (error) {
      this.readyPromise = null;
      throw error;
    }
  }

  async withFetchOverride(operation) {
    const customFetch = this.config.fetchImpl;
    if (typeof customFetch !== "function" || customFetch === globalThis.fetch) {
      return operation();
    }

    const run = async () => {
      const previousFetch = globalThis.fetch;
      globalThis.fetch = customFetch;
      try {
        return await operation();
      } finally {
        globalThis.fetch = previousFetch;
      }
    };

    this.fetchOverrideQueue = this.fetchOverrideQueue.then(run, run);
    return this.fetchOverrideQueue;
  }

  async complete(model, promptOrMessages, options = {}) {
    const rust = await this.ensureBackend();
    return this.withFetchOverride(async () => rust.complete(model, promptOrMessages, options));
  }

  async stream(model, promptOrMessages, onChunk, options = {}) {
    const rust = await this.ensureBackend();
    return this.withFetchOverride(async () => {
      const started = performance.now();
      const streamBridge = createStreamEventBridge(model, onChunk);
      const result = await rust.streamEvents(
        model,
        promptOrMessages,
        (event) => streamBridge.onEvent(event),
        options
      );
      return streamBridge.mergeResult(result, started);
    });
  }

  async streamEvents(model, promptOrMessages, onEvent, options = {}) {
    const rust = await this.ensureBackend();
    return this.withFetchOverride(async () =>
      rust.streamEvents(model, promptOrMessages, onEvent, options)
    );
  }

  async runWorkflowYamlString(yamlText, workflowInput, workflowOptions) {
    const rust = await this.ensureBackend();
    return this.withFetchOverride(async () => {
      let mergedOptions = workflowOptions;
      if (typeof this.config.fetchImpl === "function") {
        mergedOptions =
          workflowOptions && typeof workflowOptions === "object"
            ? { ...workflowOptions, __fetchImpl: this.config.fetchImpl }
            : { __fetchImpl: this.config.fetchImpl };
      }
      const result = await rust.runWorkflowYamlString(yamlText, workflowInput, mergedOptions);
      return assertWorkflowResultShape(normalizeWorkflowResult(result));
    });
  }

  async runWorkflowYaml(workflowPath, workflowInput) {
    const rust = await this.ensureBackend();
    return this.withFetchOverride(async () => {
      const result = await rust.runWorkflowYaml(workflowPath, workflowInput);
      return assertWorkflowResultShape(normalizeWorkflowResult(result));
    });
  }

  async run(request) {
    const workflowInput = buildWorkflowInputFromExecutionRequest(request);
    const workflowOptions = buildWorkflowOptionsFromExecutionRequest(request, undefined);
    return this.runWorkflowYamlString(request.workflow_yaml, workflowInput, workflowOptions);
  }

  async runAsync(request) {
    return this.run(request);
  }

  async streamWorkflow(request, onEvent) {
    const workflowInput = buildWorkflowInputFromExecutionRequest(request);
    const workflowOptions = buildWorkflowOptionsFromExecutionRequest(request, onEvent);
    return this.runWorkflowYamlString(request.workflow_yaml, workflowInput, workflowOptions);
  }
}

export async function hasRustBackend() {
  try {
    const moduleValue = await loadRustModule();
    if (!moduleValue || typeof moduleValue.supportsRustWasm !== "function") {
      return false;
    }
    return Boolean(moduleValue.supportsRustWasm());
  } catch {
    return false;
  }
}
