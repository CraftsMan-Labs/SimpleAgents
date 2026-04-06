import { parse as parseYaml } from "yaml";
import { configError, runtimeError } from "./runtime/errors.js";
import {
  applyDeltaToAggregate,
  createStreamAggregator,
  createStreamEventBridge,
  iterateSse,
  parseSseEventBlock
} from "./runtime/stream.js";
import { loadRustModule } from "./runtime/rust-runtime.js";

const DEFAULT_BASE_URLS = {
  openai: "https://api.openai.com/v1",
  openrouter: "https://openrouter.ai/api/v1"
};

function toMessages(promptOrMessages) {
  if (typeof promptOrMessages === "string") {
    const content = promptOrMessages.trim();
    if (content.length === 0) {
      throw configError("prompt cannot be empty");
    }
    return [{ role: "user", content }];
  }

  if (!Array.isArray(promptOrMessages) || promptOrMessages.length === 0) {
    throw configError("messages must be a non-empty array");
  }

  return promptOrMessages;
}

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

function toUsage(usage) {
  if (!usage || typeof usage !== "object") {
    return {
      promptTokens: 0,
      completionTokens: 0,
      totalTokens: 0
    };
  }

  return {
    promptTokens: usage.prompt_tokens ?? usage.promptTokens ?? 0,
    completionTokens: usage.completion_tokens ?? usage.completionTokens ?? 0,
    totalTokens: usage.total_tokens ?? usage.totalTokens ?? 0
  };
}

function toToolCalls(toolCalls) {
  if (!Array.isArray(toolCalls) || toolCalls.length === 0) {
    return undefined;
  }

  return toolCalls
    .filter((call) => call && typeof call === "object")
    .map((call) => ({
      id: call.id ?? "",
      toolType: call.type ?? call.toolType ?? "function",
      function: {
        name: call.function?.name ?? "",
        arguments: call.function?.arguments ?? ""
      }
    }));
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
    terminal_node: typeof result.terminal_node === "string" ? result.terminal_node : terminalNode,
    terminal_output: result.output,
    events: Array.isArray(result.events) ? result.events : [],
    status: typeof result.status === "string" ? result.status : "ok"
  };
}

function normalizeBaseUrl(baseUrl) {
  return baseUrl.replace(/\/$/, "");
}

function finiteNumberOrNull(value) {
  return Number.isFinite(value) ? value : null;
}

function buildStepDetail(step) {
  return {
    node_id: step.nodeId,
    node_kind: step.nodeKind,
    model_name: step.modelName ?? null,
    elapsed_ms: step.elapsedMs,
    prompt_tokens: finiteNumberOrNull(step.promptTokens),
    completion_tokens: finiteNumberOrNull(step.completionTokens),
    total_tokens: finiteNumberOrNull(step.totalTokens),
    reasoning_tokens: 0,
    tokens_per_second: finiteNumberOrNull(step.tokensPerSecond)
  };
}

function buildWorkflowNerdstats(summary) {
  return {
    workflow_id: summary.workflowId,
    terminal_node: summary.terminalNode,
    total_elapsed_ms: summary.totalElapsedMs,
    ttft_ms: summary.ttftMs,
    step_details: summary.stepDetails,
    total_input_tokens: summary.totalInputTokens,
    total_output_tokens: summary.totalOutputTokens,
    total_tokens: summary.totalTokens,
    total_reasoning_tokens: summary.totalReasoningTokens,
    tokens_per_second: summary.tokensPerSecond,
    trace_id: summary.traceId,
    token_metrics_available: summary.tokenMetricsAvailable,
    token_metrics_source: summary.tokenMetricsSource,
    llm_nodes_without_usage: summary.llmNodesWithoutUsage
  };
}

function interpolate(value, context) {
  if (typeof value === "string") {
    return value.replace(/{{\s*([^}]+)\s*}}/g, (_, key) => {
      const token = String(key).trim();
      const resolved = context[token];
      if (resolved === null || resolved === undefined) {
        return "";
      }
      if (typeof resolved === "string") {
        return resolved;
      }
      return JSON.stringify(resolved);
    });
  }

  if (Array.isArray(value)) {
    return value.map((entry) => interpolate(entry, context));
  }

  if (value !== null && value !== undefined && typeof value === "object") {
    const output = {};
    for (const [key, nested] of Object.entries(value)) {
      output[key] = interpolate(nested, context);
    }
    return output;
  }

  return value;
}

function evaluateCondition(condition, context) {
  if (!condition || typeof condition !== "object") {
    return false;
  }

  const left = interpolate(condition.left, context);
  const right = interpolate(condition.right, context);

  if (condition.operator === "eq") {
    return left === right;
  }
  if (condition.operator === "ne") {
    return left !== right;
  }
  if (condition.operator === "contains") {
    return String(left).includes(String(right));
  }

  return false;
}

function parseWorkflow(yamlText) {
  if (typeof yamlText !== "string" || yamlText.trim().length === 0) {
    throw configError("yamlText must be a non-empty string");
  }

  const parsed = parseYaml(yamlText);
  if (!parsed || typeof parsed !== "object") {
    throw configError("workflow YAML must parse to an object");
  }

  if (!Array.isArray(parsed.steps) && !isGraphWorkflow(parsed)) {
    throw configError(
      "workflow YAML must contain either a steps array or graph fields (entry_node + nodes)"
    );
  }

  return parsed;
}

function isGraphWorkflow(doc) {
  return (
    doc &&
    typeof doc === "object" &&
    typeof doc.entry_node === "string" &&
    Array.isArray(doc.nodes)
  );
}

function getPathValue(source, path) {
  if (!source || typeof source !== "object") {
    return undefined;
  }

  const normalized = String(path).trim().replace(/^\$\./, "");
  const tokens = normalized.split(".").filter((token) => token.length > 0);
  let current = source;
  for (const token of tokens) {
    if (!current || typeof current !== "object") {
      return undefined;
    }
    current = current[token];
  }
  return current;
}

function interpolatePathTemplate(template, context) {
  if (typeof template !== "string") {
    return "";
  }

  return template.replace(/{{\s*([^}]+)\s*}}/g, (_, token) => {
    const resolved = getPathValue(context, token);
    if (resolved === null || resolved === undefined) {
      return "";
    }
    if (typeof resolved === "string") {
      return resolved;
    }
    return JSON.stringify(resolved);
  });
}

function interpolatePathValue(value, context) {
  if (typeof value === "string") {
    return value.replace(/{{\s*([^}]+)\s*}}/g, (_, token) => {
      const resolved = getPathValue(context, token);
      if (resolved === null || resolved === undefined) {
        return "";
      }
      if (typeof resolved === "string") {
        return resolved;
      }
      return JSON.stringify(resolved);
    });
  }

  if (Array.isArray(value)) {
    return value.map((entry) => interpolatePathValue(entry, context));
  }

  if (value !== null && value !== undefined && typeof value === "object") {
    const output = {};
    for (const [key, nested] of Object.entries(value)) {
      output[key] = interpolatePathValue(nested, context);
    }
    return output;
  }

  return value;
}

function maybeParseJson(text) {
  if (typeof text !== "string") {
    return text;
  }

  try {
    return JSON.parse(text);
  } catch {
    const start = text.indexOf("{");
    const end = text.lastIndexOf("}");
    if (start !== -1 && end !== -1 && end > start) {
      const candidate = text.slice(start, end + 1);
      try {
        return JSON.parse(candidate);
      } catch {
        return text;
      }
    }
    return text;
  }
}

function matchesJsonSchemaType(expectedType, value) {
  if (expectedType === "null") {
    return value === null;
  }
  if (expectedType === "array") {
    return Array.isArray(value);
  }
  if (expectedType === "object") {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }
  if (expectedType === "integer") {
    return typeof value === "number" && Number.isInteger(value);
  }
  return typeof value === expectedType;
}

function jsonValueType(value) {
  if (value === null) {
    return "null";
  }
  if (Array.isArray(value)) {
    return "array";
  }
  return typeof value;
}

function equalJsonValue(left, right) {
  if (left === right) {
    return true;
  }
  if (
    left === null ||
    right === null ||
    left === undefined ||
    right === undefined ||
    typeof left !== "object" ||
    typeof right !== "object"
  ) {
    return false;
  }

  try {
    return JSON.stringify(left) === JSON.stringify(right);
  } catch {
    return false;
  }
}

function schemaValidationError(schema, value, path = "$") {
  if (!schema || typeof schema !== "object" || Array.isArray(schema)) {
    return null;
  }

  if (Array.isArray(schema.anyOf) && schema.anyOf.length > 0) {
    const anyOfErrors = schema.anyOf
      .map((nested) => schemaValidationError(nested, value, path))
      .filter((entry) => typeof entry === "string");
    if (anyOfErrors.length === schema.anyOf.length) {
      return `${path} did not satisfy anyOf`;
    }
  }

  if (Array.isArray(schema.oneOf) && schema.oneOf.length > 0) {
    let matched = 0;
    for (const nested of schema.oneOf) {
      if (schemaValidationError(nested, value, path) === null) {
        matched += 1;
      }
    }
    if (matched !== 1) {
      return `${path} must satisfy exactly one oneOf schema`;
    }
  }

  if (Array.isArray(schema.allOf) && schema.allOf.length > 0) {
    for (const nested of schema.allOf) {
      const error = schemaValidationError(nested, value, path);
      if (error !== null) {
        return error;
      }
    }
  }

  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    const matched = schema.enum.some((candidate) => equalJsonValue(candidate, value));
    if (!matched) {
      return `${path} must be one of enum values`;
    }
  }

  if (schema.type !== undefined) {
    const expectedTypes = Array.isArray(schema.type) ? schema.type : [schema.type];
    const matchedType = expectedTypes.some((expectedType) => {
      return typeof expectedType === "string" && matchesJsonSchemaType(expectedType, value);
    });
    if (!matchedType) {
      return `${path} expected type ${expectedTypes.join(" | ")}, got ${jsonValueType(value)}`;
    }
  }

  if (typeof value === "string") {
    if (typeof schema.minLength === "number" && value.length < schema.minLength) {
      return `${path} must have minLength ${schema.minLength}`;
    }
    if (typeof schema.maxLength === "number" && value.length > schema.maxLength) {
      return `${path} must have maxLength ${schema.maxLength}`;
    }
    if (typeof schema.pattern === "string") {
      const pattern = new RegExp(schema.pattern);
      if (!pattern.test(value)) {
        return `${path} must match pattern ${schema.pattern}`;
      }
    }
  }

  if (typeof value === "number") {
    if (typeof schema.minimum === "number" && value < schema.minimum) {
      return `${path} must be >= ${schema.minimum}`;
    }
    if (typeof schema.maximum === "number" && value > schema.maximum) {
      return `${path} must be <= ${schema.maximum}`;
    }
  }

  if (Array.isArray(value)) {
    if (typeof schema.minItems === "number" && value.length < schema.minItems) {
      return `${path} must have at least ${schema.minItems} items`;
    }
    if (typeof schema.maxItems === "number" && value.length > schema.maxItems) {
      return `${path} must have at most ${schema.maxItems} items`;
    }
    if (schema.items !== undefined) {
      for (let index = 0; index < value.length; index += 1) {
        const error = schemaValidationError(schema.items, value[index], `${path}[${index}]`);
        if (error !== null) {
          return error;
        }
      }
    }
  }

  const isObjectValue = value !== null && typeof value === "object" && !Array.isArray(value);
  if (isObjectValue) {
    const properties =
      schema.properties && typeof schema.properties === "object" && !Array.isArray(schema.properties)
        ? schema.properties
        : {};

    if (Array.isArray(schema.required)) {
      for (const key of schema.required) {
        if (typeof key !== "string") {
          continue;
        }
        if (!(key in value)) {
          return `${path}.${key} is required`;
        }
      }
    }

    for (const [key, propertySchema] of Object.entries(properties)) {
      if (!(key in value)) {
        continue;
      }
      const error = schemaValidationError(propertySchema, value[key], `${path}.${key}`);
      if (error !== null) {
        return error;
      }
    }

    const knownKeys = new Set(Object.keys(properties));
    if (schema.additionalProperties === false) {
      for (const key of Object.keys(value)) {
        if (!knownKeys.has(key)) {
          return `${path}.${key} is not allowed`;
        }
      }
    } else if (
      schema.additionalProperties !== undefined &&
      schema.additionalProperties !== true
    ) {
      for (const key of Object.keys(value)) {
        if (knownKeys.has(key)) {
          continue;
        }
        const error = schemaValidationError(
          schema.additionalProperties,
          value[key],
          `${path}.${key}`
        );
        if (error !== null) {
          return error;
        }
      }
    }
  }

  return null;
}

function llmOutputSchema(node) {
  const schema = node?.config?.output_schema;
  if (schema && typeof schema === "object" && !Array.isArray(schema)) {
    return schema;
  }
  return {
    type: "object",
    additionalProperties: true
  };
}

function evaluateSwitchCondition(condition, context) {
  if (typeof condition !== "string") {
    return false;
  }

  const eq = condition.match(/^\$\.([A-Za-z0-9_.]+)\s*==\s*"([\s\S]*)"$/);
  if (eq) {
    const left = getPathValue(context, eq[1]);
    return String(left ?? "") === eq[2];
  }

  const ne = condition.match(/^\$\.([A-Za-z0-9_.]+)\s*!=\s*"([\s\S]*)"$/);
  if (ne) {
    const left = getPathValue(context, ne[1]);
    return String(left ?? "") !== ne[2];
  }

  return false;
}

class BrowserJsClient {
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
    this.baseUrl = normalizeBaseUrl(config.baseUrl ?? DEFAULT_BASE_URLS[provider] ?? "");
    this.apiKey = config.apiKey;
    this.fetchImpl = config.fetchImpl ?? globalThis.fetch;
    this.headers = config.headers ?? {};

    if (typeof this.fetchImpl !== "function") {
      throw configError("fetch implementation is required");
    }
    if (!this.baseUrl) {
      throw configError("baseUrl is required");
    }
  }

  async complete(model, promptOrMessages, options = {}) {
    if (typeof model !== "string" || model.trim() === "") {
      throw configError("model cannot be empty");
    }

    const mode = options.mode ?? "standard";
    if (mode === "healed_json" || mode === "schema") {
      throw runtimeError(
        "healed_json and schema modes are not supported in simple-agents-wasm yet"
      );
    }

    const started = performance.now();
    const messages = toMessages(promptOrMessages);
    const response = await this.fetchImpl(`${this.baseUrl}/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${this.apiKey}`,
        ...this.headers
      },
      body: JSON.stringify({
        model,
        messages,
        max_tokens: options.maxTokens,
        temperature: options.temperature,
        top_p: options.topP,
        stream: false
      })
    });

    if (!response.ok) {
      const body = await response.text();
      throw runtimeError(`request failed (${response.status}): ${body.slice(0, 500)}`);
    }

    const data = await response.json();
    const choice = data?.choices?.[0];
    const latencyMs = Math.max(0, Math.round(performance.now() - started));

    return {
      id: data?.id ?? "",
      model: data?.model ?? model,
      role: choice?.message?.role ?? "assistant",
      content: choice?.message?.content,
      toolCalls: toToolCalls(choice?.message?.tool_calls),
      finishReason: choice?.finish_reason,
      usage: toUsage(data?.usage),
      usageAvailable: Boolean(data?.usage),
      latencyMs,
      raw: JSON.stringify(data),
      healed: undefined,
      coerced: undefined
    };
  }

  async stream(model, promptOrMessages, onChunk, options = {}) {
    if (typeof onChunk !== "function") {
      throw configError("onChunk callback is required");
    }

    const started = performance.now();
    const streamBridge = createStreamEventBridge(model, onChunk);

    const result = await this.streamEvents(
      model,
      promptOrMessages,
      (event) => streamBridge.onEvent(event),
      options
    );

    return streamBridge.mergeResult(result, started);
  }

  async streamEvents(model, promptOrMessages, onEvent, options = {}) {
    if (typeof model !== "string" || model.trim() === "") {
      throw configError("model cannot be empty");
    }
    if (typeof onEvent !== "function") {
      throw configError("onEvent callback is required");
    }

    const messages = toMessages(promptOrMessages);
    const response = await this.fetchImpl(`${this.baseUrl}/chat/completions`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${this.apiKey}`,
        ...this.headers
      },
      body: JSON.stringify({
        model,
        messages,
        max_tokens: options.maxTokens,
        temperature: options.temperature,
        top_p: options.topP,
        stream: true,
        stream_options: {
          include_usage: true
        }
      })
    });

    if (!response.ok) {
      const body = await response.text();
      const message = `request failed (${response.status}): ${body.slice(0, 500)}`;
      const errorEvent = { eventType: "error", error: { message } };
      onEvent(errorEvent);
      throw runtimeError(message);
    }

    const started = performance.now();
    const aggregateState = createStreamAggregator(model);
    let usage = {
      promptTokens: 0,
      completionTokens: 0,
      totalTokens: 0
    };
    let usageAvailable = false;

    try {
      for await (const block of iterateSse(response)) {
        const parsed = parseSseEventBlock(block);
        if (!parsed) {
          continue;
        }

        if (parsed.done) {
          break;
        }

        if (!parsed.json) {
          continue;
        }

        const chunk = parsed.json;
        const choice = chunk?.choices?.[0];
        const delta = {
          id: chunk?.id ?? "",
          model: chunk?.model ?? model,
          index: choice?.index ?? 0,
          role: choice?.delta?.role,
          content: choice?.delta?.content,
          finishReason: choice?.finish_reason,
          raw: parsed.raw
        };

        applyDeltaToAggregate(aggregateState, delta);
        if (chunk?.usage && typeof chunk.usage === "object") {
          usage = toUsage(chunk.usage);
          usageAvailable = true;
        }

        onEvent({ eventType: "delta", delta });
      }

      onEvent({ eventType: "done" });
    } catch (error) {
      const message = error instanceof Error ? error.message : "stream parsing failed";
      onEvent({ eventType: "error", error: { message } });
      throw runtimeError(message);
    }

    const latencyMs = Math.max(0, Math.round(performance.now() - started));

    return {
      id: aggregateState.responseId,
      model: aggregateState.responseModel,
      role: "assistant",
      content: aggregateState.aggregate,
      finishReason: aggregateState.finishReason,
      usage: {
        ...usage
      },
      usageAvailable,
      latencyMs,
      raw: undefined,
      healed: undefined,
      coerced: undefined
    };
  }

  async runWorkflowYamlString() {
    const yamlText = arguments[0];
    const workflowInput = arguments[1] ?? {};
    const workflowOptions = arguments[2] ?? {};

    const doc = parseWorkflow(yamlText);
    if (workflowInput === null || typeof workflowInput !== "object") {
      throw configError("workflowInput must be an object");
    }

    const context = { ...workflowInput };
    const events = [];
    const functions =
      workflowOptions && typeof workflowOptions.functions === "object"
        ? workflowOptions.functions
        : {};

    if (isGraphWorkflow(doc)) {
      const nodeById = new Map();
      for (const node of doc.nodes) {
        if (!node || typeof node !== "object" || typeof node.id !== "string") {
          throw configError("workflow node is invalid or missing id");
        }
        nodeById.set(node.id, node);
      }

      const edgeMap = new Map();
      if (Array.isArray(doc.edges)) {
        for (const edge of doc.edges) {
          if (!edge || typeof edge.from !== "string" || typeof edge.to !== "string") {
            continue;
          }
          const existing = edgeMap.get(edge.from) ?? [];
          existing.push(edge.to);
          edgeMap.set(edge.from, existing);
        }
      }

      const graphContext = {
        input: workflowInput,
        nodes: {}
      };

      const workflowStarted = performance.now();
      let workflowTtftMs = 0;
      let pointer = doc.entry_node;
      let output;
      let iterations = 0;
      const stepDetails = [];
      let totalInputTokens = 0;
      let totalOutputTokens = 0;
      let totalTokens = 0;
      const totalReasoningTokens = 0;
      const llmNodesWithoutUsage = [];

      while (typeof pointer === "string" && pointer.length > 0) {
        iterations += 1;
        if (iterations > 1000) {
          throw runtimeError("workflow exceeded maximum step iterations");
        }

        const node = nodeById.get(pointer);
        if (!node) {
          throw configError(`workflow references unknown node '${pointer}'`);
        }

        const nodeType = node.node_type ?? {};
        const nodeTypeName = Object.keys(nodeType)[0] ?? "unknown";
        const nodeStarted = performance.now();
        let stepModelName;
        let stepPromptTokens;
        let stepCompletionTokens;
        let stepTotalTokens;
        events.push({ stepId: node.id, stepType: nodeTypeName, status: "started" });

        if (nodeType.llm_call) {
          const llm = nodeType.llm_call;
          const model = llm.model ?? doc.model ?? workflowInput.model;
          if (typeof model !== "string" || model.trim().length === 0) {
            throw configError(`llm_call node '${node.id}' requires node_type.llm_call.model`);
          }

          const prompt = interpolatePathTemplate(node.config?.prompt ?? "", graphContext);
          let promptOrMessages = prompt;
          if (llm.messages_path === "input.messages") {
            const source = getPathValue(graphContext, llm.messages_path);
            const history = Array.isArray(source)
              ? source
                  .filter((message) => {
                    return (
                      message &&
                      typeof message === "object" &&
                      typeof message.role === "string" &&
                      typeof message.content === "string"
                    );
                  })
                  .map((message) => ({ role: message.role, content: message.content }))
              : [];
            if (llm.append_prompt_as_user !== false) {
              history.push({ role: "user", content: prompt });
            }
            promptOrMessages = history;
          }

          let rawContent = "";
          let completion;
          if (llm.stream === true) {
            completion = await this.streamEvents(
              model,
              promptOrMessages,
              (event) => {
                if (event && event.eventType === "delta" && typeof event.delta?.content === "string") {
                  if (workflowTtftMs === 0) {
                    const measured = Math.max(0, Math.round(performance.now() - workflowStarted));
                    workflowTtftMs = measured === 0 ? 1 : measured;
                  }
                  rawContent += event.delta.content;
                  if (typeof workflowOptions?.onEvent === "function") {
                    workflowOptions.onEvent({
                      eventType: "node_stream_delta",
                      nodeId: node.id,
                      delta: event.delta.content,
                      model: event.delta.model ?? model
                    });
                  }
                }
              },
              {
                temperature: llm.temperature
              }
            );
            rawContent = completion.content ?? rawContent;
          } else {
            completion = await this.complete(model, promptOrMessages, {
              temperature: llm.temperature
            });
            rawContent = completion.content ?? "";
          }

          stepModelName = completion?.model ?? model;
          if (completion?.usageAvailable === true && completion?.usage && typeof completion.usage === "object") {
            const usage = completion.usage;
            stepPromptTokens = Number.isFinite(usage.promptTokens) ? usage.promptTokens : 0;
            stepCompletionTokens = Number.isFinite(usage.completionTokens)
              ? usage.completionTokens
              : 0;
            stepTotalTokens = Number.isFinite(usage.totalTokens) ? usage.totalTokens : 0;
            totalInputTokens += stepPromptTokens;
            totalOutputTokens += stepCompletionTokens;
            totalTokens += stepTotalTokens;
          } else {
            llmNodesWithoutUsage.push(node.id);
          }

          const parsedOutput = maybeParseJson(rawContent);
          const validationError = schemaValidationError(llmOutputSchema(node), parsedOutput);
          if (validationError !== null) {
            throw runtimeError(
              `llm_call node '${node.id}' output failed schema validation: ${validationError}`
            );
          }
          graphContext.nodes[node.id] = {
            output: parsedOutput,
            raw: rawContent
          };
          output = parsedOutput;

          const nextTargets = edgeMap.get(node.id) ?? [];
          pointer = nextTargets[0] ?? "";
        } else if (nodeType.switch) {
          const switchSpec = nodeType.switch;
          const branches = Array.isArray(switchSpec.branches) ? switchSpec.branches : [];
          const matched = branches.find((branch) =>
            evaluateSwitchCondition(branch?.condition, graphContext)
          );
          pointer = matched?.target ?? switchSpec.default ?? "";
        } else if (nodeType.custom_worker) {
          const handler = nodeType.custom_worker.handler ?? "custom_worker";
          const handlerFile = nodeType.custom_worker.handler_file;
          const lookupKey =
            typeof handlerFile === "string" && handlerFile.length > 0
              ? `${handlerFile}#${handler}`
              : handler;
          const fn = functions[lookupKey];
          if (typeof fn !== "function") {
            throw runtimeError(
              `custom_worker node '${node.id}' requires workflowOptions.functions['${lookupKey}']`
            );
          }
          const workerOutput = await fn(
            {
              handler,
              handler_file: handlerFile,
              handler_lookup_key: lookupKey,
              payload: interpolatePathValue(node.config?.payload ?? null, graphContext),
              nodeId: node.id
            },
            graphContext
          );
          graphContext.nodes[node.id] = {
            output: workerOutput
          };
          output = workerOutput;
          const nextTargets = edgeMap.get(node.id) ?? [];
          pointer = nextTargets[0] ?? "";
        } else {
          throw configError(`unsupported node_type in simple-agents-wasm graph workflow`);
        }

        events.push({ stepId: node.id, stepType: nodeTypeName, status: "completed" });
        const elapsedMs = Math.max(0, Math.round(performance.now() - nodeStarted));
        const stepTokensPerSecond =
          Number.isFinite(stepCompletionTokens) && elapsedMs > 0
            ? Math.round((stepCompletionTokens / (elapsedMs / 1000)) * 100) / 100
            : null;
        stepDetails.push(
          buildStepDetail({
            nodeId: node.id,
            nodeKind: nodeTypeName,
            modelName: stepModelName,
            elapsedMs,
            promptTokens: stepPromptTokens,
            completionTokens: stepCompletionTokens,
            totalTokens: stepTotalTokens,
            tokensPerSecond: stepTokensPerSecond
          })
        );
      }

      const trace = events
        .filter((event) => event && event.status === "completed")
        .map((event) => event.stepId);
      const terminalNode = trace.at(-1) ?? "";
      const totalElapsedMs = Math.max(0, Math.round(performance.now() - workflowStarted));
      const tokenMetricsAvailable = llmNodesWithoutUsage.length === 0;
      const overallTokensPerSecond =
        totalElapsedMs > 0 ? Math.round((totalOutputTokens / (totalElapsedMs / 1000)) * 100) / 100 : 0;
      const workflowId =
        typeof doc.id === "string" && doc.id.length > 0 ? doc.id : "browser_js_workflow";
      const nerdstats = buildWorkflowNerdstats({
        workflowId,
        terminalNode,
        totalElapsedMs,
        ttftMs: workflowTtftMs,
        stepDetails,
        totalInputTokens,
        totalOutputTokens,
        totalTokens,
        totalReasoningTokens,
        tokensPerSecond: overallTokensPerSecond,
        traceId: "",
        tokenMetricsAvailable,
        tokenMetricsSource: tokenMetricsAvailable ? "provider_usage" : "unavailable",
        llmNodesWithoutUsage
      });
      if (typeof workflowOptions?.onEvent === "function") {
        workflowOptions.onEvent({
          event_type: "workflow_completed",
          metadata: {
            nerdstats
          }
        });
      }

      const outputs = {};
      for (const [nodeId, nodeValue] of Object.entries(graphContext.nodes ?? {})) {
        if (nodeValue && typeof nodeValue === "object" && "output" in nodeValue) {
          outputs[nodeId] = nodeValue.output;
        } else {
          outputs[nodeId] = nodeValue;
        }
      }

      return {
        workflow_id: workflowId,
        entry_node: doc.entry_node,
        email_text: typeof graphContext.input?.email_text === "string" ? graphContext.input.email_text : "",
        trace,
        outputs,
        terminal_node: terminalNode,
        terminal_output: output,
        step_timings: stepDetails,
        total_elapsed_ms: totalElapsedMs,
        ttft_ms: workflowTtftMs,
        total_input_tokens: totalInputTokens,
        total_output_tokens: totalOutputTokens,
        total_tokens: totalTokens,
        total_reasoning_tokens: totalReasoningTokens,
        tokens_per_second: overallTokensPerSecond,
        trace_id: "",
        metadata: {
          nerdstats
        },
        events,
        context: graphContext,
        status: "ok"
      };
    }

    const indexById = new Map();
    doc.steps.forEach((step, index) => {
      if (!step || typeof step !== "object") {
        throw configError(`workflow step at index ${index} must be an object`);
      }
      if (!step.id || !step.type) {
        throw configError(`workflow step at index ${index} requires id and type`);
      }
      indexById.set(step.id, index);
    });

    let pointer = 0;
    let output;
    let iterations = 0;

    while (pointer < doc.steps.length) {
      iterations += 1;
      if (iterations > 1000) {
        throw runtimeError("workflow exceeded maximum step iterations");
      }

      const step = doc.steps[pointer];
      events.push({ stepId: step.id, stepType: step.type, status: "started" });

      if (step.type === "set") {
        if (typeof step.key !== "string" || step.key.length === 0) {
          throw configError(`set step '${step.id}' requires key`);
        }
        context[step.key] = interpolate(step.value, context);
      } else if (step.type === "llm_call") {
        const prompt = String(interpolate(step.prompt ?? "", context));
        const model = step.model ?? doc.model ?? context.model;
        if (typeof model !== "string" || model.trim().length === 0) {
          throw configError(`llm_call step '${step.id}' requires a model (step.model, workflow model, or workflowInput.model)`);
        }
        const completion = await this.complete(model, prompt, {
          temperature: step.temperature
        });
        context[step.id] = completion.content ?? "";
      } else if (step.type === "if") {
        const matched = evaluateCondition(step.condition, context);
        const targetId = matched ? step.then : step.else;
        if (targetId) {
          const jumpTo = indexById.get(targetId);
          if (jumpTo === undefined) {
            throw configError(`if step '${step.id}' points to unknown step '${targetId}'`);
          }
          events.push({ stepId: step.id, stepType: step.type, status: "completed" });
          pointer = jumpTo;
          continue;
        }
      } else if (step.type === "call_function") {
        if (typeof step.function !== "string" || step.function.length === 0) {
          throw configError(`call_function step '${step.id}' requires function`);
        }

        const fn = functions[step.function];
        if (typeof fn !== "function") {
          throw configError(`call_function step '${step.id}' references unknown function '${step.function}'`);
        }

        const args = interpolate(step.args ?? {}, context);
        context[step.id] = await fn(args, context);
      } else if (step.type === "output") {
        output = interpolate(step.text ?? step.value ?? "", context);
        context[step.id] = output;
      } else {
        throw configError(`unsupported step type '${step.type}' in simple-agents-wasm`);
      }

      events.push({ stepId: step.id, stepType: step.type, status: "completed" });

      if (step.next) {
        const jumpTo = indexById.get(step.next);
        if (jumpTo === undefined) {
          throw configError(`step '${step.id}' points to unknown next step '${step.next}'`);
        }
        pointer = jumpTo;
        continue;
      }

      pointer += 1;
    }

    return {
      workflow_id:
        typeof doc.id === "string" && doc.id.length > 0 ? doc.id : "browser_js_workflow",
      entry_node: typeof doc.steps?.[0]?.id === "string" ? doc.steps[0].id : "",
      email_text: typeof workflowInput?.email_text === "string" ? workflowInput.email_text : "",
      trace: events
        .filter((event) => event && event.status === "completed")
        .map((event) => event.stepId),
      outputs: { ...context },
      terminal_node: events
        .filter((event) => event && event.status === "completed")
        .map((event) => event.stepId)
        .at(-1) ?? "",
      terminal_output: output,
      events,
      context,
      status: "ok"
    };
  }

  async runWorkflowYaml(workflowPath) {
    throw runtimeError(
      `workflow file paths are not supported in browser runtime: ${workflowPath}`
    );
  }
}

export class Client {
  constructor(provider, config) {
    this.fallbackClient = new BrowserJsClient(provider, config);
    this.provider = provider;
    this.config = config;
    this.rustClient = null;
    this.readyPromise = null;
  }

  async ensureBackend() {
    if (this.rustClient) {
      return this.rustClient;
    }
    if (this.readyPromise) {
      return this.readyPromise;
    }

    this.readyPromise = (async () => {
      if (this.config.fetchImpl && this.config.fetchImpl !== globalThis.fetch) {
        return null;
      }

      const moduleValue = await loadRustModule();
      if (!moduleValue || typeof moduleValue.WasmClient !== "function") {
        return null;
      }

      try {
        const client = new moduleValue.WasmClient(this.provider, {
          apiKey: this.config.apiKey,
          baseUrl: this.config.baseUrl,
          headers: this.config.headers
        });
        this.rustClient = client;
        return client;
      } catch {
        return null;
      }
    })();

    return this.readyPromise;
  }

  async complete(model, promptOrMessages, options = {}) {
    const rust = await this.ensureBackend();
    if (rust) {
      return rust.complete(model, promptOrMessages, options);
    }
    return this.fallbackClient.complete(model, promptOrMessages, options);
  }

  async stream(model, promptOrMessages, onChunk, options = {}) {
    const rust = await this.ensureBackend();
    if (rust) {
      const started = performance.now();
      const streamBridge = createStreamEventBridge(model, onChunk);

      const result = await rust.streamEvents(
        model,
        promptOrMessages,
        (event) => streamBridge.onEvent(event),
        options
      );

      return streamBridge.mergeResult(result, started);
    }

    return this.fallbackClient.stream(model, promptOrMessages, onChunk, options);
  }

  async streamEvents(model, promptOrMessages, onEvent, options = {}) {
    const rust = await this.ensureBackend();
    if (rust) {
      return rust.streamEvents(model, promptOrMessages, onEvent, options);
    }
    return this.fallbackClient.streamEvents(model, promptOrMessages, onEvent, options);
  }

  async runWorkflowYamlString(yamlText, workflowInput, workflowOptions) {
    const rust = await this.ensureBackend();
    if (rust) {
      const result = await rust.runWorkflowYamlString(yamlText, workflowInput, workflowOptions);
      return assertWorkflowResultShape(normalizeWorkflowResult(result));
    }
    const result = await this.fallbackClient.runWorkflowYamlString(
      yamlText,
      workflowInput,
      workflowOptions
    );
    return assertWorkflowResultShape(normalizeWorkflowResult(result));
  }

  async runWorkflowYaml(workflowPath, workflowInput) {
    const rust = await this.ensureBackend();
    if (rust) {
      const result = await rust.runWorkflowYaml(workflowPath, workflowInput);
      return assertWorkflowResultShape(normalizeWorkflowResult(result));
    }
    const result = await this.fallbackClient.runWorkflowYaml(workflowPath, workflowInput);
    return assertWorkflowResultShape(normalizeWorkflowResult(result));
  }

  async run(request) {
    const workflowInput = buildWorkflowInputFromExecutionRequest(request);
    const workflowOptions = buildWorkflowOptionsFromExecutionRequest(request, undefined);
    return this.runWorkflowYamlString(request.workflow_yaml, workflowInput, workflowOptions);
  }

  async runAsync(request) {
    return this.run(request);
  }

  // Workflow-streaming surface (completion streaming already uses `stream`).
  async streamWorkflow(request, onEvent) {
    const workflowInput = buildWorkflowInputFromExecutionRequest(request);
    const workflowOptions = buildWorkflowOptionsFromExecutionRequest(request, onEvent);
    return this.runWorkflowYamlString(request.workflow_yaml, workflowInput, workflowOptions);
  }
}

export async function hasRustBackend() {
  const moduleValue = await loadRustModule();
  if (!moduleValue || typeof moduleValue.supportsRustWasm !== "function") {
    return false;
  }

  try {
    return Boolean(moduleValue.supportsRustWasm());
  } catch {
    return false;
  }
}
