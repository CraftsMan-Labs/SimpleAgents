import { parse as parseYaml } from "yaml";

const DEFAULT_BASE_URLS = {
  openai: "https://api.openai.com/v1",
  openrouter: "https://openrouter.ai/api/v1"
};

function configError(message) {
  return new Error(`simple-agents-wasm config error: ${message}`);
}

function runtimeError(message) {
  return new Error(`simple-agents-wasm runtime error: ${message}`);
}

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

function createStreamEventBridge(model, onChunk) {
  let aggregate = "";
  let finalId = "";
  let finalModel = model;
  let finalFinishReason;

  return {
    onEvent(event) {
      if (event.eventType === "delta") {
        const delta = event.delta;
        if (!delta) {
          return;
        }

        if (!finalId && delta.id) {
          finalId = delta.id;
        }
        if (delta.model) {
          finalModel = delta.model;
        }
        if (delta.content) {
          aggregate += delta.content;
        }
        if (delta.finishReason) {
          finalFinishReason = delta.finishReason;
        }

        onChunk({
          id: delta.id,
          model: delta.model,
          content: delta.content,
          finishReason: delta.finishReason,
          raw: delta.raw
        });
      }

      if (event.eventType === "error") {
        onChunk({
          id: finalId || "error",
          model: finalModel,
          error: event.error?.message ?? "stream error"
        });
      }
    },
    mergeResult(result, started) {
      return {
        ...result,
        id: result.id || finalId,
        model: result.model || finalModel,
        content: result.content ?? aggregate,
        finishReason: result.finishReason ?? finalFinishReason,
        latencyMs: Math.max(0, Math.round(performance.now() - started))
      };
    }
  };
}

function normalizeBaseUrl(baseUrl) {
  return baseUrl.replace(/\/$/, "");
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

function normalizeSseChunk(chunk) {
  return chunk.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
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

function parseSseEventBlock(block) {
  const lines = block.split("\n");
  const dataLines = [];
  for (const line of lines) {
    if (line.startsWith("data:")) {
      dataLines.push(line.slice(5).trimStart());
    }
  }

  if (dataLines.length === 0) {
    return null;
  }

  const payload = dataLines.join("\n");
  if (payload === "[DONE]") {
    return { done: true };
  }

  try {
    return { done: false, json: JSON.parse(payload), raw: payload };
  } catch {
    return { done: false, raw: payload };
  }
}

async function* iterateSse(response) {
  if (!response.body) {
    throw runtimeError("stream response had no body");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  while (true) {
    const { value, done } = await reader.read();
    if (done) {
      break;
    }

    buffer += normalizeSseChunk(decoder.decode(value, { stream: true }));
    let delimiterIndex = buffer.indexOf("\n\n");
    while (delimiterIndex !== -1) {
      const block = buffer.slice(0, delimiterIndex).trim();
      buffer = buffer.slice(delimiterIndex + 2);
      if (block.length > 0) {
        yield block;
      }
      delimiterIndex = buffer.indexOf("\n\n");
    }
  }

  buffer += normalizeSseChunk(decoder.decode());

  const trailing = buffer.trim();
  if (trailing.length > 0) {
    yield trailing;
  }
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
        stream: true
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
    let responseId = "";
    let responseModel = model;
    let aggregate = "";
    let finishReason;

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

        if (!responseId && delta.id) {
          responseId = delta.id;
        }
        if (delta.model) {
          responseModel = delta.model;
        }
        if (delta.content) {
          aggregate += delta.content;
        }
        if (delta.finishReason) {
          finishReason = delta.finishReason;
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
      id: responseId,
      model: responseModel,
      role: "assistant",
      content: aggregate,
      finishReason,
      usage: {
        promptTokens: 0,
        completionTokens: 0,
        totalTokens: 0
      },
      usageAvailable: false,
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

      let pointer = doc.entry_node;
      let output;
      let iterations = 0;

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

          const completion = await this.complete(model, promptOrMessages, {
            temperature: llm.temperature
          });
          const parsedOutput = maybeParseJson(completion.content ?? "");
          const validationError = schemaValidationError(llmOutputSchema(node), parsedOutput);
          if (validationError !== null) {
            throw runtimeError(
              `llm_call node '${node.id}' output failed schema validation: ${validationError}`
            );
          }
          graphContext.nodes[node.id] = {
            output: parsedOutput,
            raw: completion.content ?? ""
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
          const fn = functions[handler];
          if (typeof fn !== "function") {
            throw runtimeError(
              `custom_worker node '${node.id}' requires workflowOptions.functions['${handler}']`
            );
          }
          const workerOutput = await fn(
            {
              handler,
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
      }

      return {
        status: "ok",
        context: graphContext,
        output,
        events
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
      status: "ok",
      context,
      output,
      events
    };
  }

  async runWorkflowYaml(workflowPath) {
    throw runtimeError(
      `workflow file paths are not supported in browser runtime: ${workflowPath}`
    );
  }
}

let rustModulePromise;

async function loadRustModule() {
  if (!rustModulePromise) {
    rustModulePromise = (async () => {
      try {
        const moduleValue = await import("./pkg/simple_agents_wasm.js");
        const wasmUrl = new URL("./pkg/simple_agents_wasm_bg.wasm", import.meta.url);
        await moduleValue.default(wasmUrl);
        return moduleValue;
      } catch {
        return null;
      }
    })();
  }

  return rustModulePromise;
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
      return rust.runWorkflowYamlString(yamlText, workflowInput, workflowOptions);
    }
    return this.fallbackClient.runWorkflowYamlString(yamlText, workflowInput, workflowOptions);
  }

  async runWorkflowYaml(workflowPath, workflowInput) {
    const rust = await this.ensureBackend();
    if (rust) {
      return rust.runWorkflowYaml(workflowPath, workflowInput);
    }
    return this.fallbackClient.runWorkflowYaml(workflowPath, workflowInput);
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
