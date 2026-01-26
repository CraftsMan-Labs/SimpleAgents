# simple-agents-napi

Node.js bindings for SimpleAgents using napi-rs.

## Build

```sh
npm install
npm run build
```

## Usage

```javascript
const { Client } = require('./index.node');

const client = new Client('openai');
const response = client.complete('gpt-4', 'Hello from Node!', 128, 0.7);
console.log(response);
```

## Notes

- `Client` reads provider configuration from environment variables (e.g. `OPENAI_API_KEY`).
- `max_tokens` and `temperature` are optional.
