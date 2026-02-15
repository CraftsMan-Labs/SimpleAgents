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
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

## Notes

- Canonical env contract for examples/tests is: `CUSTOM_API_BASE`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, `PROVIDER`.
- Canonical env contract for examples/tests is: `PROVIDER`, `CUSTOM_API_KEY`, `CUSTOM_API_BASE` (optional), `CUSTOM_API_MODEL`.
- For OpenAI provider compatibility, map `CUSTOM_API_*` to `OPENAI_API_*` when needed (`OPENAI_API_KEY`, `OPENAI_API_BASE`, `OPENAI_MODEL`).
- `max_tokens`, `temperature`, and `top_p` are optional. Use `mode: "healed_json"` for parsed JSON or `mode: "schema"` with a schema object to coerce/validate.
- `complete` resolves with the first choice, usage metadata, and optional `healed`/`coerced` metadata.
- `stream` invokes a chunk callback and resolves with aggregated content (healing/schema not yet supported for streams).
- `streamEvents` is the canonical typed streaming callback API with `delta`, `error`, and `done` events.
- Set `CUSTOM_API_BASE`, `CUSTOM_API_KEY`, `CUSTOM_API_MODEL`, and `PROVIDER` to run tests/examples consistently across bindings.
