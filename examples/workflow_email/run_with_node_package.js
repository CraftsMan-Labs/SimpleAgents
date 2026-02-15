const fs = require('node:fs')
const path = require('node:path')

function loadDotEnv(filePath) {
  if (!fs.existsSync(filePath)) return
  const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/)
  for (const line of lines) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const idx = trimmed.indexOf('=')
    if (idx <= 0) continue
    const key = trimmed.slice(0, idx).trim()
    const value = trimmed.slice(idx + 1).trim()
    if (!process.env[key]) process.env[key] = value
  }
}

function mapProviderEnv(provider, apiKey, apiBase) {
  if (provider === 'openai') {
    process.env.OPENAI_API_KEY = apiKey
    if (apiBase) process.env.OPENAI_API_BASE = apiBase
    return
  }
  if (provider === 'anthropic') {
    process.env.ANTHROPIC_API_KEY = apiKey
    return
  }
  if (provider === 'openrouter') {
    process.env.OPENROUTER_API_KEY = apiKey
    if (apiBase) process.env.OPENROUTER_API_BASE = apiBase
    return
  }
  throw new Error(`Unsupported WORKFLOW_PROVIDER: ${provider}`)
}

async function main() {
  loadDotEnv(path.resolve(__dirname, '../.env'))

  const provider = process.env.WORKFLOW_PROVIDER || 'openai'
  const apiBase = process.env.WORKFLOW_API_BASE || process.env.CUSTOM_API_BASE
  const apiKey = process.env.WORKFLOW_API_KEY || process.env.CUSTOM_API_KEY

  if (!apiKey) {
    throw new Error('Set WORKFLOW_API_KEY or CUSTOM_API_KEY')
  }

  mapProviderEnv(provider, apiKey, apiBase)

  const { Client } = require('../../crates/simple-agents-napi')
  const client = new Client(provider)

  const workflowPath = process.argv[2] || 'examples/workflow_email/email-intake-classification.yaml'
  const emailText = process.argv[3] || 'Please process supply chain replacement, order 9921 arrived damaged.'

  const result = client.runEmailWorkflowYaml(workflowPath, emailText)
  console.log(JSON.stringify(result, null, 2))
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
