const fs = require('node:fs')
const path = require('node:path')

function loadDotEnv(filePath) {
  if (!fs.existsSync(filePath)) {
    return
  }

  const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/)
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed === '' || trimmed.startsWith('#')) {
      continue
    }
    const index = trimmed.indexOf('=')
    if (index <= 0) {
      continue
    }
    const key = trimmed.slice(0, index).trim()
    const value = trimmed.slice(index + 1).trim()
    if (process.env[key] === undefined) {
      process.env[key] = value
    }
  }
}

function mapProviderEnv(provider, apiKey, apiBase) {
  if (provider === 'openai') {
    process.env.OPENAI_API_KEY = apiKey
    if (apiBase !== '') {
      process.env.OPENAI_API_BASE = apiBase
    }
    return
  }

  if (provider === 'anthropic') {
    process.env.ANTHROPIC_API_KEY = apiKey
    return
  }

  if (provider === 'openrouter') {
    process.env.OPENROUTER_API_KEY = apiKey
    if (apiBase !== '') {
      process.env.OPENROUTER_API_BASE = apiBase
    }
    return
  }

  throw new Error(`Unsupported WORKFLOW_PROVIDER: ${provider}`)
}

function listWorkflows(baseDir) {
  return fs
    .readdirSync(baseDir)
    .filter((name) => name.endsWith('.yaml'))
    .map((name) => path.join(baseDir, name))
    .sort()
}

function run() {
  const rootDir = path.resolve(__dirname, '..')
  loadDotEnv(path.resolve(rootDir, '../.env'))

  const provider = process.env.WORKFLOW_PROVIDER || 'openai'
  const apiBase = process.env.WORKFLOW_API_BASE || process.env.CUSTOM_API_BASE || ''
  const apiKey = process.env.WORKFLOW_API_KEY || process.env.CUSTOM_API_KEY || ''
  if (apiKey === '') {
    throw new Error('Set WORKFLOW_API_KEY or CUSTOM_API_KEY')
  }

  mapProviderEnv(provider, apiKey, apiBase)

  const { Client } = require('../../../crates/simple-agents-napi')
  const client = new Client(provider)

  const emailText =
    process.argv[2] ||
    'Please help with a damaged supply order and draft the right response.'
  const workflows = listWorkflows(rootDir)
  const workflowInput = {
    email_text: emailText,
    messages: [
      {
        role: 'system',
        content: 'You are a professional assistant for workflow testing.',
      },
      { role: 'user', content: emailText },
    ],
  }

  const summary = []
  for (const workflowPath of workflows) {
    try {
      const result = client.runWorkflowYaml(workflowPath, workflowInput)
      summary.push({
        workflow: workflowPath,
        status: 'ok',
        terminal_node: result.terminal_node,
        total_elapsed_ms: result.total_elapsed_ms,
      })
    } catch (error) {
      summary.push({
        workflow: workflowPath,
        status: 'error',
        error: error instanceof Error ? error.message : String(error),
      })
    }
  }

  console.log(JSON.stringify(summary, null, 2))
}

try {
  run()
} catch (error) {
  console.error(error)
  process.exitCode = 1
}
