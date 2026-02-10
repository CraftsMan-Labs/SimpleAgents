// Basic Node.js example using the napi bindings.
// Build the addon first:
//   cd crates/simple-agents-napi && npm install && npm run build
// Then run:
//   node examples/node_client.js

const path = require('path');
// eslint-disable-next-line import/no-dynamic-require, global-require
const { Client } = require(path.join(__dirname, '../crates/simple-agents-napi'));

if (!process.env.OPENAI_API_KEY && process.env.CUSTOM_API_KEY) {
  process.env.OPENAI_API_KEY = process.env.CUSTOM_API_KEY;
}
if (!process.env.OPENAI_API_BASE && process.env.CUSTOM_API_BASE) {
  process.env.OPENAI_API_BASE = process.env.CUSTOM_API_BASE;
}

const MODEL = process.env.CUSTOM_API_MODEL;
const PROVIDER = process.env.PROVIDER;
if (!process.env.OPENAI_API_KEY || !MODEL || !PROVIDER) {
  console.error(
    'Set CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL, and PROVIDER (openai|anthropic|openrouter) before running this example.',
  );
  process.exit(1);
}

async function main() {
  const client = new Client(PROVIDER);

  const completion = await client.complete(
    MODEL,
    [
      { role: 'system', content: 'You are concise.' },
      { role: 'user', content: 'Give me one fun project idea.' },
    ],
    { max_tokens: 64 },
  );
  console.log('\ncompletion:', completion.content);
  console.log('usage:', completion.usage);

  console.log('\nstreaming:');
  await client.stream(
    MODEL,
    'Say hello in five words.',
    (chunk) => {
      if (!chunk || typeof chunk !== 'object') {
        return;
      }
      if (chunk.content) process.stdout.write(chunk.content);
      if (chunk.error) console.error('error:', chunk.error);
      if (chunk.finishReason) console.log('\nfinishReason:', chunk.finishReason);
    },
    {},
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
