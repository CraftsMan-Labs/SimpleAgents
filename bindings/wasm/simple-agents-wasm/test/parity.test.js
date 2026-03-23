import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { Client } from "../index.js";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const fixturePath = resolve(
  __dirname,
  "../../../../parity-fixtures/node_wasm_completion_shape.json"
);
const shapeFixture = JSON.parse(readFileSync(fixturePath, "utf8"));

function makeJsonResponse(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" }
  });
}

function makeSseResponse(events, status = 200) {
  const stream = new ReadableStream({
    start(controller) {
      events.forEach((event) => {
        controller.enqueue(new TextEncoder().encode(`data: ${event}\n\n`));
      });
      controller.close();
    }
  });

  return new Response(stream, {
    status,
    headers: { "Content-Type": "text/event-stream" }
  });
}

test("complete() returns parity shape keys", async () => {
  const mockFetch = async () =>
    makeJsonResponse({
      id: "cmpl_1",
      model: "gpt-4o-mini",
      usage: { prompt_tokens: 7, completion_tokens: 4, total_tokens: 11 },
      choices: [
        {
          finish_reason: "stop",
          message: { role: "assistant", content: "hello", tool_calls: [] }
        }
      ]
    });

  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: mockFetch
  });

  const result = await client.complete("gpt-4o-mini", "hi");

  for (const key of shapeFixture.completionResultKeys) {
    assert.ok(key in result, `expected key '${key}' in completion result`);
  }
  for (const key of shapeFixture.usageKeys) {
    assert.ok(key in result.usage, `expected usage key '${key}'`);
  }
  assert.equal(result.content, "hello");
  assert.equal(result.finishReason, "stop");
  assert.equal(result.usage.totalTokens, 11);
});

test("streamEvents() emits delta + done with parity event shape", async () => {
  const mockFetch = async () =>
    makeSseResponse([
      JSON.stringify({
        id: "chatcmpl_1",
        model: "gpt-4o-mini",
        choices: [{ index: 0, delta: { role: "assistant", content: "he" } }]
      }),
      JSON.stringify({
        id: "chatcmpl_1",
        model: "gpt-4o-mini",
        choices: [{ index: 0, delta: { content: "llo" }, finish_reason: "stop" }]
      }),
      "[DONE]"
    ]);

  const events = [];
  const client = new Client("openai", {
    apiKey: "test-key",
    baseUrl: "https://example.com/v1",
    fetchImpl: mockFetch
  });

  const result = await client.streamEvents("gpt-4o-mini", "say hello", (event) => {
    events.push(event);
  });

  assert.equal(events.at(-1).eventType, "done");
  assert.equal(result.content, "hello");

  const deltaEvent = events.find((event) => event.eventType === "delta");
  assert.ok(deltaEvent, "expected at least one delta event");
  assert.ok(shapeFixture.streamEventTypes.includes(deltaEvent.eventType));
  for (const key of shapeFixture.streamDeltaKeys) {
    assert.ok(key in deltaEvent.delta, `expected stream delta key '${key}'`);
  }
});
