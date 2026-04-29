const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { Client } = require('..');

/** Valid-format dummy key (never used for network when workflow has no llm_call). */
const DUMMY_API_KEY = 'sk-1234567890abcdef1234567890';

function writeCustomOnlyWorkflow(t) {
  const workflowPath = path.join(os.tmpdir(), `napi-cw-only-${Date.now()}.yaml`);
  const yaml = `id: napi-custom-worker-contract
version: 1.0.0
entry_node: worker
nodes:
  - id: worker
    node_type:
      custom_worker:
        handler: echo_handler
    config:
      payload:
        company_name: "acme"
`;
  fs.writeFileSync(workflowPath, yaml, 'utf8');
  t.after(() => {
    try {
      fs.unlinkSync(workflowPath);
    } catch (_err) {
      // no-op
    }
  });
  return workflowPath;
}

function dispatchEcho(req) {
  if (req.handler === 'echo_handler') {
    return { stakeholder_name: `echo:${String((req.payload && req.payload.company_name) || '')}` };
  }
  throw new Error(`unknown handler ${req.handler}`);
}

/** `resume` / `runWorkflow` return a value or a Promise when `customWorker` is used. */
async function awaitRunLike(p) {
  return typeof p?.then === 'function' ? p : Promise.resolve(p);
}

test('runWorkflow rejects custom_worker workflow when customWorkerDispatch is omitted', (t) => {
  const workflowPath = writeCustomOnlyWorkflow(t);
  const client = new Client(DUMMY_API_KEY);
  const messages = [{ role: 'user', content: 'hi' }];

  assert.throws(
    () => {
      client.runWorkflow(workflowPath, { messages });
    },
    (err) =>
      err != null &&
      String(err.message || err).includes('no custom worker executor is configured'),
    'expected startup validation error when no executor is registered',
  );
});

test('runWorkflow executes custom_worker when customWorkerDispatch is provided', async (t) => {
  const workflowPath = writeCustomOnlyWorkflow(t);
  const client = new Client(DUMMY_API_KEY);
  const messages = [{ role: 'user', content: 'hi' }];

  const result = await awaitRunLike(
    client.runWorkflow(workflowPath, { messages }, undefined, undefined, dispatchEcho),
  );

  assert.ok(result && typeof result === 'object');
  assert.strictEqual(result.terminal_node, 'worker');
  assert.ok(result.outputs && typeof result.outputs === 'object');
  const workerOut = result.outputs.worker;
  assert.ok(workerOut && typeof workerOut === 'object');
  assert.strictEqual(workerOut.output.stakeholder_name, 'echo:acme');
});

test('streamWorkflow without onEvent rejects when customWorkerDispatch is omitted', (t) => {
  const workflowPath = writeCustomOnlyWorkflow(t);
  const client = new Client(DUMMY_API_KEY);
  const messages = [{ role: 'user', content: 'hi' }];

  assert.throws(
    () => {
      client.streamWorkflow(workflowPath, { messages }, () => undefined);
    },
    (err) =>
      err != null &&
      String(err.message || err).includes('no custom worker executor is configured'),
  );
});

test('streamWorkflow with onEvent and customWorkerDispatch resolves', async (t) => {
  const workflowPath = writeCustomOnlyWorkflow(t);
  const client = new Client(DUMMY_API_KEY);
  const messages = [{ role: 'user', content: 'hi' }];
  const events = [];

  const result = await client.streamWorkflow(
    workflowPath,
    { messages },
    (eventJson) => {
      events.push(eventJson);
    },
    undefined,
    undefined,
    dispatchEcho,
  );

  assert.ok(result && typeof result === 'object');
  assert.strictEqual(result.terminal_node, 'worker');
  assert.ok(result.outputs && result.outputs.worker);
  assert.strictEqual(result.outputs.worker.output.stakeholder_name, 'echo:acme');
  assert.ok(events.length >= 1, 'expected at least one workflow event');
});

test('customWorkerDispatch throw surfaces as workflow error', async (t) => {
  const workflowPath = writeCustomOnlyWorkflow(t);
  const client = new Client(DUMMY_API_KEY);
  const messages = [{ role: 'user', content: 'hi' }];

  await assert.rejects(
    async () => {
      await awaitRunLike(
        client.runWorkflow(
          workflowPath,
          { messages },
          undefined,
          undefined,
          () => {
            throw new Error('boom');
          },
        ),
      );
    },
    (err) => err != null && String(err.message || err).includes('boom'),
  );
});
