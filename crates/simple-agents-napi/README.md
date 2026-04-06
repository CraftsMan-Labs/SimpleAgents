# simple-agents-napi

Node.js bindings for SimpleAgents using napi-rs.

## Build

```sh
npm install
npm run build
```

## Usage

```javascript
const { Client } = require('./index.js');

const provider = process.env.PROVIDER;
const model = process.env.CUSTOM_API_MODEL;
const key = process.env.CUSTOM_API_KEY;
const base = process.env.CUSTOM_API_BASE;

if (!provider || !model || !key) {
  throw new Error('Set PROVIDER, CUSTOM_API_KEY, and CUSTOM_API_MODEL.');
}

if (provider === 'openai') {
  process.env.OPENAI_API_KEY = key;
  if (base) process.env.OPENAI_API_BASE = base;
}
if (provider === 'anthropic') {
  process.env.ANTHROPIC_API_KEY = key;
}
if (provider === 'openrouter') {
  process.env.OPENROUTER_API_KEY = key;
  if (base) process.env.OPENROUTER_API_BASE = base;
}

const client = new Client(provider);

async function main() {
  const response = await client.complete(
    model,
    [
      { role: 'system', content: 'You are concise.' },
      { role: 'user', content: 'Say hi from Node.' },
    ],
    { max_tokens: 64, temperature: 0.7 },
  );

  console.log(response.content);
  console.log(response.usage);
  console.log(response.healed); // present when using healed_json/schema mode

  // Streaming (legacy chunk callback)
  const streamed = await client.stream(
    model,
    'Say hello in two words.',
    (chunk) => {
      if (chunk.content) process.stdout.write(chunk.content);
      if (chunk.finish_reason) console.log('\nfinish:', chunk.finish_reason);
    },
    {},
  );
  console.log('streamed content:', streamed.content);

  // Streaming (typed event callback)
  await client.streamEvents(
    model,
    'Say hello in two words.',
    (event) => {
      if (event.eventType === 'delta' && event.delta?.content) {
        process.stdout.write(event.delta.content);
      }
      if (event.eventType === 'error') {
        console.error('\nstream error:', event.error?.message);
      }
      if (event.eventType === 'done') {
        console.log('\nstream done');
      }
    },
    {},
  );

  // Healed JSON
  const healed = await client.complete(
    'gpt-4',
    'Respond with JSON: {"message": "hello"}',
    { mode: 'healed_json' },
  );
  console.log('parsed JSON value:', healed.healed?.value);

  // Workflow YAML (sync)
  const workflowOutput = client.runWorkflowYaml(
    'examples/workflow_email/email-intake-classification.yaml',
    {
      messages: [
        { role: 'system', content: 'You are an HR classifier.' },
        { role: 'user', content: 'Termination request, second warning already issued' },
      ],
    },
    { include_events: true },
  );
  console.log('terminal output:', workflowOutput.terminal_output);

  // Workflow YAML (async + live events)
  const asyncOutput = await client.runWorkflowYamlStream(
    'examples/workflow_email/email-intake-classification.yaml',
    {
      messages: [
        { role: 'system', content: 'You are an HR classifier.' },
        { role: 'user', content: 'Termination request, second warning already issued' },
      ],
    },
    (err, eventJson) => {
      if (err) {
        console.error('event error:', err);
        return;
      }
      console.log('event:', eventJson);
    },
    { telemetry: { nerdstats: true } },
  );
  console.log('async terminal output:', asyncOutput.terminal_output);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

## Notes

- `client.executeWorkflowYaml` / `executeWorkflowYamlStream` accept typed `workflowOptions` (`WorkflowRunOptionsNapi`: `model`, `telemetry`, `trace`, `includeEvents`). For stream events, `require('simple-agents-node/workflow_event').parseWorkflowEvent(eventJson)` parses the JSON string into a typed object.
- Canonical env contract for examples/tests is: `CUSTOM_API_BASE`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, `PROVIDER`.
- Canonical env contract for examples/tests is: `PROVIDER`, `CUSTOM_API_KEY`, `CUSTOM_API_BASE` (optional), `CUSTOM_API_MODEL`.
- For OpenAI provider compatibility, map `CUSTOM_API_*` to `OPENAI_API_*` when needed (`OPENAI_API_KEY`, `OPENAI_API_BASE`, `OPENAI_MODEL`).
- `max_tokens`, `temperature`, and `top_p` are optional. Use `mode: "healed_json"` for parsed JSON or `mode: "schema"` with a schema object to coerce/validate.
- `complete` resolves with the first choice, usage metadata, and optional `healed`/`coerced` metadata.
- `stream` invokes a chunk callback and resolves with aggregated content (healing/schema not yet supported for streams).
- `streamEvents` is the canonical typed streaming callback API with `delta`, `error`, and `done` events.
- Set `CUSTOM_API_BASE`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, and `PROVIDER` to run tests/examples consistently across bindings.

## Test Layers

- `npm run test:unit` - runtime/export sanity checks
- `npm run test:contract` - shared fixture parity contract checks
- `npm run test:live` - provider-backed live checks (env-gated)
