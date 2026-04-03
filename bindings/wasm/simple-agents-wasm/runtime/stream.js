import { runtimeError } from "./errors.js";

export function createStreamAggregator(model) {
  return {
    responseId: "",
    responseModel: model,
    aggregate: "",
    finishReason: undefined
  };
}

export function applyDeltaToAggregate(state, delta) {
  if (!state || !delta) {
    return;
  }

  if (!state.responseId && delta.id) {
    state.responseId = delta.id;
  }
  if (delta.model) {
    state.responseModel = delta.model;
  }
  if (delta.content) {
    state.aggregate += delta.content;
  }
  if (delta.finishReason) {
    state.finishReason = delta.finishReason;
  }
}

export function createStreamEventBridge(model, onChunk) {
  const aggregateState = createStreamAggregator(model);

  return {
    onEvent(event) {
      if (event.eventType === "delta") {
        const delta = event.delta;
        if (!delta) {
          return;
        }

        applyDeltaToAggregate(aggregateState, delta);

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
          id: aggregateState.responseId || "error",
          model: aggregateState.responseModel,
          error: event.error?.message ?? "stream error"
        });
      }
    },
    mergeResult(result, started) {
      return {
        ...result,
        id: result.id || aggregateState.responseId,
        model: result.model || aggregateState.responseModel,
        content: result.content ?? aggregateState.aggregate,
        finishReason: result.finishReason ?? aggregateState.finishReason,
        latencyMs: Math.max(0, Math.round(performance.now() - started))
      };
    }
  };
}

function normalizeSseChunk(chunk) {
  return chunk.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
}

export function parseSseEventBlock(block) {
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

export async function* iterateSse(response) {
  if (!response.body) {
    throw runtimeError("stream response had no body");
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  try {
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
  } finally {
    reader.releaseLock();
  }
}
