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

test('runEvalSuite camel-cases native eval report fields', async () => {
  class Client {
    runEvalSuite() {
      return {
        suite_id: 'friendly',
        status: 'failed',
        summary: {
          total_cases: 2,
          passed_cases: 1,
          failed_cases: 1,
          error_cases: 0,
          pass_rate: 0.5,
        },
        cases: [
          {
            case_id: 'case-1',
            status: 'failed',
            first_failed_node: 'final',
            first_failed_path: '$.outputs.final.answer',
            expected: { answer: 'yes' },
            actual: { answer: 'no' },
            workflow_output: { terminal_node: 'final' },
            error: null,
          },
        ],
      };
    }
  }

  const binding = loadWrapperWithNative({ Client });
  const report = await new binding.Client().runEvalSuite({ suitePath: 'eval.yaml' });

  assert.strictEqual(report.suiteId, 'friendly');
  assert.strictEqual(report.summary.totalCases, 2);
  assert.strictEqual(report.summary.passedCases, 1);
  assert.strictEqual(report.summary.failedCases, 1);
  assert.strictEqual(report.summary.errorCases, 0);
  assert.strictEqual(report.summary.passRate, 0.5);
  assert.strictEqual(report.cases[0].caseId, 'case-1');
  assert.strictEqual(report.cases[0].firstFailedNode, 'final');
  assert.strictEqual(report.cases[0].firstFailedPath, '$.outputs.final.answer');
  assert.strictEqual(report.cases[0].workflowOutput.terminal_node, 'final');
  assert.strictEqual(report.suite_id, undefined);
  assert.strictEqual(report.summary.total_cases, undefined);
  assert.strictEqual(report.cases[0].first_failed_path, undefined);
  assert.strictEqual(report.cases[0].workflow_output, undefined);
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
