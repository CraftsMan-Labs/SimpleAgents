const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

function loadWrapperWithNative(native) {
  const filename = path.join(__dirname, '..', 'index.js');
  const dirname = path.dirname(filename);
  const source = fs.readFileSync(filename, 'utf8');
  const module = { exports: {} };

  function localRequire(specifier) {
    if (specifier === './index.node') {
      return native;
    }
    return require(specifier);
  }

  vm.runInNewContext(source, {
    __dirname: dirname,
    __filename: filename,
    module,
    exports: module.exports,
    require: localRequire,
  }, { filename });

  return module.exports;
}

test('runEvalSuite runs dataset records through evaluator callback', async () => {
  const datasetPath = path.join(__dirname, 'tmp-eval.dataset.jsonl');
  fs.writeFileSync(
    datasetPath,
    `${JSON.stringify({
      id: 'case-1',
      input: { messages: [{ role: 'user', content: 'hi' }] },
      expected_output: { terminal_node: 'final' },
    })}\n`,
    'utf8',
  );

  class Client {
    runWorkflow(_workflowPath, workflowInput) {
      assert.strictEqual(JSON.stringify(workflowInput.messages), JSON.stringify([{ role: 'user', content: 'hi' }]));
      return { terminal_node: 'final', terminal_output: { ok: true } };
    }
  }

  const binding = loadWrapperWithNative({ Client });
  const report = await new binding.Client().runEvalSuite({
    workflowPath: 'workflow.yaml',
    datasetPath,
    suiteId: 'friendly',
    evaluator: ({ expectedOutput, actualOutput }) => ({
      id: 'terminal_node',
      status: expectedOutput.terminal_node === actualOutput.terminal_node ? 'passed' : 'failed',
      passed: expectedOutput.terminal_node === actualOutput.terminal_node,
    }),
  });

  assert.strictEqual(report.suiteId, 'friendly');
  assert.strictEqual(report.status, 'passed');
  assert.strictEqual(report.summary.totalCases, 1);
  assert.strictEqual(report.summary.passedCases, 1);
  assert.strictEqual(report.summary.failedCases, 0);
  assert.strictEqual(report.summary.errorCases, 0);
  assert.strictEqual(report.summary.passRate, 1);
  assert.strictEqual(report.cases[0].caseId, 'case-1');
  assert.strictEqual(report.cases[0].evaluations[0].id, 'terminal_node');
  assert.strictEqual(report.cases[0].workflowOutput.terminal_node, 'final');

  fs.rmSync(datasetPath);
});

test('wrapper installs workflow YAML compatibility aliases', () => {
  class Client {
    runWorkflow() {}
    streamWorkflow() {}
    run() {}
    stream() {}
  }

  const binding = loadWrapperWithNative({ Client });
  const aliases = [
    'runWorkflowYaml',
    'runWorkflowYamlWithEvents',
    'runWorkflowYamlStream',
    'executeWorkflowYaml',
    'executeWorkflowYamlStream',
  ];

  for (const alias of aliases) {
    assert.strictEqual(typeof binding.Client.prototype[alias], 'function');
  }
});

test('typed workflow request wrappers reject unknown keys', () => {
  class Client {
    runWorkflow() {}
    streamWorkflow() {}
  }

  const binding = loadWrapperWithNative({
    Client,
    parseWorkflowYamlExecutionRequest() {
      throw new Error('parser should not be reached');
    },
  });

  const client = new binding.Client();
  assert.throws(
    () => client.run({ workflowPath: 'workflow.yaml', messages: [], typo: true }),
    /unknown key "typo"/,
  );
  assert.throws(
    () => client.stream({ workflowPath: 'workflow.yaml', messages: [], typo: true }, () => {}),
    /unknown key "typo"/,
  );
});
