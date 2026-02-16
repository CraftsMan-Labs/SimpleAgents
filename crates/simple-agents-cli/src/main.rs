use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use simple_agent_type::prelude::{ApiKey, CompletionRequest, Message, SimpleAgentsError};
use simple_agents_core::{
    CompletionOptions, CompletionOutcome, RoutingMode, SimpleAgentsClient,
    SimpleAgentsClientBuilder,
};
use simple_agents_providers::{
    anthropic::AnthropicProvider, openai::OpenAIProvider, openrouter::OpenRouterProvider, Provider,
};
use simple_agents_router::{
    CostRouterConfig, FallbackRouterConfig, LatencyRouterConfig, ProviderCost,
};
use simple_agents_workflow::{
    inspect_replay_trace, replay_trace_with_options, workflow_to_mermaid, yaml_workflow_file_to_mermaid,
    ReplayCachePolicy, ReplayOptions, WorkflowDefinition, WorkflowTrace,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};

#[derive(Parser, Debug)]
#[command(name = "simple-agents", version, about = "SimpleAgents CLI")]
struct Cli {
    /// Path to a TOML/YAML configuration file
    #[arg(long)]
    config: Option<PathBuf>,
    /// Output format (plain, json, markdown)
    #[arg(long, value_enum)]
    output: Option<OutputFormat>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run a single completion request
    Complete(CompleteArgs),
    /// Start an interactive chat session
    Chat(ChatArgs),
    /// Benchmark a prompt across multiple runs
    Benchmark(BenchmarkArgs),
    /// Test provider health with a lightweight prompt
    TestProvider(TestProviderArgs),
    /// Workflow trace and replay utilities
    Workflow(WorkflowArgs),
}

#[derive(Args, Debug)]
struct WorkflowArgs {
    #[command(subcommand)]
    command: WorkflowCommands,
}

#[derive(Subcommand, Debug)]
enum WorkflowCommands {
    /// Print events from a recorded workflow trace JSON file
    Trace { trace_file: PathBuf },
    /// Replay-validate a workflow trace JSON file
    Replay {
        trace_file: PathBuf,
        #[arg(long, value_enum, default_value_t = WorkflowCachePolicyArg::Refresh)]
        cache_policy: WorkflowCachePolicyArg,
    },
    /// Inspect replay violations scoped to one node id
    Inspect {
        trace_file: PathBuf,
        node_id: String,
    },
    /// Render a workflow file as Mermaid flowchart (YAML or IR JSON)
    Mermaid { workflow_file: PathBuf },
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum WorkflowCachePolicyArg {
    Always,
    Refresh,
    Mixed,
}

#[derive(Args, Debug)]
struct CompleteArgs {
    /// Prompt to send to the model
    prompt: String,
    /// Model identifier to use
    #[arg(long)]
    model: Option<String>,
    /// Provider to use (openai, anthropic, openrouter)
    #[arg(long)]
    provider: Option<String>,
    /// Optional system prompt
    #[arg(long)]
    system: Option<String>,
    /// Maximum tokens to generate
    #[arg(long)]
    max_tokens: Option<u32>,
    /// Sampling temperature (0.0-2.0)
    #[arg(long)]
    temperature: Option<f32>,
    /// Nucleus sampling threshold (0.0-1.0)
    #[arg(long)]
    top_p: Option<f32>,
    /// User identifier for provider analytics
    #[arg(long)]
    user: Option<String>,
}

#[derive(Args, Debug)]
struct ChatArgs {
    /// Model identifier to use
    #[arg(long)]
    model: Option<String>,
    /// Provider to use (openai, anthropic, openrouter)
    #[arg(long)]
    provider: Option<String>,
    /// Optional system prompt
    #[arg(long)]
    system: Option<String>,
    /// Maximum tokens to generate
    #[arg(long)]
    max_tokens: Option<u32>,
    /// Sampling temperature (0.0-2.0)
    #[arg(long)]
    temperature: Option<f32>,
    /// Nucleus sampling threshold (0.0-1.0)
    #[arg(long)]
    top_p: Option<f32>,
    /// User identifier for provider analytics
    #[arg(long)]
    user: Option<String>,
}

#[derive(Args, Debug)]
struct BenchmarkArgs {
    /// Prompt to send to the model
    prompt: String,
    /// Number of benchmark runs
    #[arg(long, default_value_t = 10)]
    runs: u32,
    /// Number of warmup runs (not included in stats)
    #[arg(long, default_value_t = 0)]
    warmup: u32,
    /// Model identifier to use
    #[arg(long)]
    model: Option<String>,
    /// Provider to use (openai, anthropic, openrouter)
    #[arg(long)]
    provider: Option<String>,
    /// Optional system prompt
    #[arg(long)]
    system: Option<String>,
    /// Maximum tokens to generate
    #[arg(long)]
    max_tokens: Option<u32>,
    /// Sampling temperature (0.0-2.0)
    #[arg(long)]
    temperature: Option<f32>,
    /// Nucleus sampling threshold (0.0-1.0)
    #[arg(long)]
    top_p: Option<f32>,
    /// User identifier for provider analytics
    #[arg(long)]
    user: Option<String>,
}

#[derive(Args, Debug)]
struct TestProviderArgs {
    /// Prompt to send for the health check
    #[arg(long, default_value = "ping")]
    prompt: String,
    /// Model identifier to use
    #[arg(long)]
    model: Option<String>,
    /// Provider to test (openai, anthropic, openrouter)
    #[arg(long)]
    provider: Option<String>,
    /// Optional system prompt
    #[arg(long)]
    system: Option<String>,
    /// Maximum tokens to generate
    #[arg(long)]
    max_tokens: Option<u32>,
    /// Sampling temperature (0.0-2.0)
    #[arg(long)]
    temperature: Option<f32>,
    /// Nucleus sampling threshold (0.0-1.0)
    #[arg(long)]
    top_p: Option<f32>,
    /// User identifier for provider analytics
    #[arg(long)]
    user: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OutputFormat {
    Plain,
    Json,
    Markdown,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProviderKind {
    OpenAI,
    Anthropic,
    OpenRouter,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderEntry {
    kind: ProviderKind,
    api_key: Option<String>,
    api_key_env: Option<String>,
    base_url: Option<String>,
    default_model: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct Defaults {
    model: Option<String>,
    system: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    user: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RoutingConfig {
    mode: Option<RoutingModeKind>,
    latency: Option<LatencyConfig>,
    cost: Option<Vec<CostConfig>>,
    fallback: Option<FallbackConfig>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RoutingModeKind {
    Direct,
    RoundRobin,
    Latency,
    Cost,
    Fallback,
}

#[derive(Debug, Clone, Deserialize)]
struct LatencyConfig {
    alpha: Option<f64>,
    slow_threshold_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct CostConfig {
    name: String,
    cost_per_1k_tokens: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct FallbackConfig {
    retryable_only: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct OutputConfig {
    format: OutputFormat,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct ConfigFile {
    providers: Option<Vec<ProviderEntry>>,
    defaults: Option<Defaults>,
    routing: Option<RoutingConfig>,
    output: Option<OutputConfig>,
}

#[derive(Debug, Clone, Default)]
struct RequestOverrides {
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    user: Option<String>,
}

#[derive(Clone)]
struct ConfiguredProvider {
    entry: ProviderEntry,
    provider: std::sync::Arc<dyn Provider>,
}

#[derive(Debug, Serialize)]
struct BenchmarkSummary {
    runs: u32,
    warmup: u32,
    avg_ms: f64,
    min_ms: f64,
    max_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    total_tokens_avg: Option<f64>,
}

#[derive(Debug, Serialize)]
struct ProviderTestResult {
    provider: String,
    success: bool,
    latency_ms: Option<f64>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProviderTestReport {
    results: Vec<ProviderTestResult>,
}

#[derive(Error, Debug)]
enum CliError {
    #[error("config error: {0}")]
    Config(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("{0}")]
    Core(#[from] SimpleAgentsError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(String),
}

type Result<T> = std::result::Result<T, CliError>;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{}", err);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config(cli.config.as_deref())?;
    let output = resolve_output(cli.output, config.as_ref());

    match cli.command {
        Commands::Complete(args) => {
            let defaults = config.as_ref().and_then(|cfg| cfg.defaults.clone());
            let providers = resolve_providers(config.as_ref())?;
            let providers = filter_providers(providers, args.provider.as_deref())?;
            let model = resolve_model(args.model.as_deref(), defaults.as_ref(), &providers)?;
            let overrides = resolve_overrides(
                defaults.as_ref(),
                &args.user,
                args.max_tokens,
                args.temperature,
                args.top_p,
            );
            let system = args
                .system
                .or_else(|| defaults.as_ref().and_then(|d| d.system.clone()));
            let client = build_client(&providers, config.as_ref())?;
            let response =
                execute_completion(&client, &model, &args.prompt, system.as_deref(), &overrides)
                    .await?;
            print_completion(output, &response, true)?;
        }
        Commands::Chat(args) => {
            let defaults = config.as_ref().and_then(|cfg| cfg.defaults.clone());
            let providers = resolve_providers(config.as_ref())?;
            let providers = filter_providers(providers, args.provider.as_deref())?;
            let model = resolve_model(args.model.as_deref(), defaults.as_ref(), &providers)?;
            let overrides = resolve_overrides(
                defaults.as_ref(),
                &args.user,
                args.max_tokens,
                args.temperature,
                args.top_p,
            );
            let system = args
                .system
                .or_else(|| defaults.as_ref().and_then(|d| d.system.clone()));
            let client = build_client(&providers, config.as_ref())?;
            run_chat(&client, &model, system.as_deref(), &overrides, output).await?;
        }
        Commands::Benchmark(args) => {
            let defaults = config.as_ref().and_then(|cfg| cfg.defaults.clone());
            let providers = resolve_providers(config.as_ref())?;
            let providers = filter_providers(providers, args.provider.as_deref())?;
            let model = resolve_model(args.model.as_deref(), defaults.as_ref(), &providers)?;
            let overrides = resolve_overrides(
                defaults.as_ref(),
                &args.user,
                args.max_tokens,
                args.temperature,
                args.top_p,
            );
            let system = args
                .system
                .or_else(|| defaults.as_ref().and_then(|d| d.system.clone()));
            let client = build_client(&providers, config.as_ref())?;
            let summary = run_benchmark(
                &client,
                &model,
                &args.prompt,
                system.as_deref(),
                &overrides,
                args.runs,
                args.warmup,
            )
            .await?;
            print_benchmark(output, &summary)?;
        }
        Commands::TestProvider(args) => {
            let defaults = config.as_ref().and_then(|cfg| cfg.defaults.clone());
            let providers = resolve_providers(config.as_ref())?;
            let providers = filter_providers(providers, args.provider.as_deref())?;
            let overrides = resolve_overrides(
                defaults.as_ref(),
                &args.user,
                args.max_tokens,
                args.temperature,
                args.top_p,
            );
            let system = args
                .system
                .or_else(|| defaults.as_ref().and_then(|d| d.system.clone()));
            let report = run_provider_tests(
                &providers,
                config.as_ref(),
                &args.prompt,
                args.model.as_deref(),
                defaults.as_ref(),
                system.as_deref(),
                &overrides,
            )
            .await?;
            print_provider_report(output, &report)?;
        }
        Commands::Workflow(args) => {
            run_workflow_tools(args, output)?;
        }
    }

    Ok(())
}

fn load_config(path: Option<&Path>) -> Result<Option<ConfigFile>> {
    let Some(path) = path else {
        return Ok(None);
    };

    let contents = std::fs::read_to_string(path)?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let config = match ext.as_str() {
        "toml" => toml::from_str(&contents)
            .map_err(|err| CliError::Config(format!("failed to parse TOML: {}", err)))?,
        "yaml" | "yml" => serde_yaml::from_str(&contents)
            .map_err(|err| CliError::Config(format!("failed to parse YAML: {}", err)))?,
        _ => {
            return Err(CliError::Config(
                "unsupported config format (use .toml, .yaml, or .yml)".to_string(),
            ))
        }
    };

    Ok(Some(config))
}

fn resolve_output(cli_output: Option<OutputFormat>, config: Option<&ConfigFile>) -> OutputFormat {
    cli_output
        .or_else(|| config.and_then(|cfg| cfg.output.as_ref().map(|out| out.format)))
        .unwrap_or(OutputFormat::Plain)
}

fn resolve_providers(config: Option<&ConfigFile>) -> Result<Vec<ProviderEntry>> {
    if let Some(config) = config {
        if let Some(providers) = config.providers.clone() {
            if providers.is_empty() {
                return Err(CliError::Config("no providers configured".to_string()));
            }
            return Ok(providers);
        }
    }

    let mut providers = Vec::new();

    if std::env::var("OPENAI_API_KEY").is_ok() {
        providers.push(ProviderEntry {
            kind: ProviderKind::OpenAI,
            api_key: None,
            api_key_env: None,
            base_url: std::env::var("OPENAI_API_BASE").ok(),
            default_model: None,
        });
    }

    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        providers.push(ProviderEntry {
            kind: ProviderKind::Anthropic,
            api_key: None,
            api_key_env: None,
            base_url: std::env::var("ANTHROPIC_API_BASE")
                .or_else(|_| std::env::var("ANTHROPIC__API_BASE"))
                .ok(),
            default_model: None,
        });
    }

    if std::env::var("OPENROUTER_API_KEY").is_ok() {
        providers.push(ProviderEntry {
            kind: ProviderKind::OpenRouter,
            api_key: None,
            api_key_env: None,
            base_url: std::env::var("OPENROUTER_API_BASE").ok(),
            default_model: None,
        });
    }

    if providers.is_empty() {
        return Err(CliError::Config(
            "no providers configured (set a config file or API key env vars)".to_string(),
        ));
    }

    Ok(providers)
}

fn filter_providers(
    providers: Vec<ProviderEntry>,
    requested: Option<&str>,
) -> Result<Vec<ProviderEntry>> {
    let Some(requested) = requested else {
        return Ok(providers);
    };

    let requested = requested.to_lowercase();
    let filtered: Vec<ProviderEntry> = providers
        .into_iter()
        .filter(|entry| entry.name() == requested)
        .collect();

    if filtered.is_empty() {
        return Err(CliError::Config(format!(
            "provider '{}' not found",
            requested
        )));
    }

    Ok(filtered)
}

fn resolve_model(
    cli_model: Option<&str>,
    defaults: Option<&Defaults>,
    providers: &[ProviderEntry],
) -> Result<String> {
    if let Some(model) = cli_model {
        return Ok(model.to_string());
    }

    if let Some(defaults) = defaults {
        if let Some(model) = &defaults.model {
            return Ok(model.clone());
        }
    }

    if providers.len() == 1 {
        if let Some(model) = &providers[0].default_model {
            return Ok(model.clone());
        }
    }

    Err(CliError::Config(
        "model is required (use --model or set defaults.model in config)".to_string(),
    ))
}

fn resolve_overrides(
    defaults: Option<&Defaults>,
    user: &Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
) -> RequestOverrides {
    let defaults = defaults.cloned();
    let default_user = defaults.as_ref().and_then(|d| d.user.clone());
    RequestOverrides {
        max_tokens: max_tokens.or_else(|| defaults.as_ref().and_then(|d| d.max_tokens)),
        temperature: temperature.or_else(|| defaults.as_ref().and_then(|d| d.temperature)),
        top_p: top_p.or_else(|| defaults.as_ref().and_then(|d| d.top_p)),
        user: user.clone().or(default_user),
    }
}

fn build_client(
    providers: &[ProviderEntry],
    config: Option<&ConfigFile>,
) -> Result<SimpleAgentsClient> {
    let configured = build_provider_instances(providers)?;
    let routing = resolve_routing(config)?;
    let mut builder = SimpleAgentsClientBuilder::new().with_routing_mode(routing);
    for provider in configured {
        builder = builder.with_provider(provider.provider);
    }
    Ok(builder.build()?)
}

fn resolve_routing(config: Option<&ConfigFile>) -> Result<RoutingMode> {
    let Some(config) = config else {
        return Ok(RoutingMode::RoundRobin);
    };

    let Some(routing) = config.routing.clone() else {
        return Ok(RoutingMode::RoundRobin);
    };

    let mode = routing.mode.unwrap_or(RoutingModeKind::RoundRobin);

    match mode {
        RoutingModeKind::Direct => Ok(RoutingMode::Direct),
        RoutingModeKind::RoundRobin => Ok(RoutingMode::RoundRobin),
        RoutingModeKind::Latency => {
            let mut config = LatencyRouterConfig::default();
            if let Some(latency) = routing.latency {
                if let Some(alpha) = latency.alpha {
                    config.alpha = alpha;
                }
                if let Some(ms) = latency.slow_threshold_ms {
                    config.slow_threshold = Duration::from_millis(ms);
                }
            }
            Ok(RoutingMode::Latency(config))
        }
        RoutingModeKind::Cost => {
            let cost_entries = routing
                .cost
                .unwrap_or_default()
                .into_iter()
                .map(|entry| ProviderCost::new(entry.name, entry.cost_per_1k_tokens))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|err| CliError::Config(err.to_string()))?;
            Ok(RoutingMode::Cost(CostRouterConfig::new(cost_entries)))
        }
        RoutingModeKind::Fallback => {
            let retryable_only = routing
                .fallback
                .and_then(|fallback| fallback.retryable_only)
                .unwrap_or(true);
            Ok(RoutingMode::Fallback(FallbackRouterConfig {
                retryable_only,
            }))
        }
    }
}

fn build_provider_instances(providers: &[ProviderEntry]) -> Result<Vec<ConfiguredProvider>> {
    let mut configured = Vec::with_capacity(providers.len());
    for entry in providers {
        let api_key = resolve_api_key(entry)?;
        let provider: std::sync::Arc<dyn Provider> = match entry.kind {
            ProviderKind::OpenAI => {
                let provider = match entry.base_url.clone() {
                    Some(base_url) => OpenAIProvider::with_base_url(api_key, base_url)
                        .map_err(|err| CliError::Provider(err.to_string()))?,
                    None => OpenAIProvider::new(api_key)
                        .map_err(|err| CliError::Provider(err.to_string()))?,
                };
                std::sync::Arc::new(provider)
            }
            ProviderKind::Anthropic => {
                let provider = match entry.base_url.clone() {
                    Some(base_url) => AnthropicProvider::with_base_url(api_key, base_url)
                        .map_err(|err| CliError::Provider(err.to_string()))?,
                    None => AnthropicProvider::new(api_key)
                        .map_err(|err| CliError::Provider(err.to_string()))?,
                };
                std::sync::Arc::new(provider)
            }
            ProviderKind::OpenRouter => {
                let provider = match entry.base_url.clone() {
                    Some(base_url) => OpenRouterProvider::with_base_url(api_key, base_url)
                        .map_err(|err| CliError::Provider(err.to_string()))?,
                    None => OpenRouterProvider::new(api_key)
                        .map_err(|err| CliError::Provider(err.to_string()))?,
                };
                std::sync::Arc::new(provider)
            }
        };

        configured.push(ConfiguredProvider {
            entry: entry.clone(),
            provider,
        });
    }

    Ok(configured)
}

fn resolve_api_key(entry: &ProviderEntry) -> Result<ApiKey> {
    if let Some(api_key) = &entry.api_key {
        return ApiKey::new(api_key.clone()).map_err(CliError::from);
    }

    if let Some(env_key) = &entry.api_key_env {
        let value = std::env::var(env_key)
            .map_err(|_| CliError::Config(format!("missing API key in env var {}", env_key)))?;
        return ApiKey::new(value).map_err(CliError::from);
    }

    let env_name = match entry.kind {
        ProviderKind::OpenAI => "OPENAI_API_KEY",
        ProviderKind::Anthropic => "ANTHROPIC_API_KEY",
        ProviderKind::OpenRouter => "OPENROUTER_API_KEY",
    };

    let value = std::env::var(env_name)
        .map_err(|_| CliError::Config(format!("missing API key in env var {}", env_name)))?;
    ApiKey::new(value).map_err(CliError::from)
}

async fn execute_completion(
    client: &SimpleAgentsClient,
    model: &str,
    prompt: &str,
    system: Option<&str>,
    overrides: &RequestOverrides,
) -> Result<simple_agent_type::response::CompletionResponse> {
    let messages = build_messages(prompt, system);
    let request = build_request(model, messages, overrides)?;
    let outcome = client
        .complete(&request, CompletionOptions::default())
        .await?;
    match outcome {
        CompletionOutcome::Response(response) => Ok(response),
        CompletionOutcome::Stream(_) => Err(CliError::Config(
            "streaming response returned from non-streaming call".to_string(),
        )),
        CompletionOutcome::HealedJson(_) => Err(CliError::Config(
            "healed json response returned from non-streaming call".to_string(),
        )),
        CompletionOutcome::CoercedSchema(_) => Err(CliError::Config(
            "schema response returned from non-streaming call".to_string(),
        )),
    }
}

fn build_messages(prompt: &str, system: Option<&str>) -> Vec<Message> {
    let mut messages = Vec::new();
    if let Some(system) = system {
        messages.push(Message::system(system));
    }
    messages.push(Message::user(prompt));
    messages
}

fn build_request(
    model: &str,
    messages: Vec<Message>,
    overrides: &RequestOverrides,
) -> Result<CompletionRequest> {
    let mut builder = CompletionRequest::builder().model(model).messages(messages);
    if let Some(max_tokens) = overrides.max_tokens {
        builder = builder.max_tokens(max_tokens);
    }
    if let Some(temperature) = overrides.temperature {
        builder = builder.temperature(temperature);
    }
    if let Some(top_p) = overrides.top_p {
        builder = builder.top_p(top_p);
    }
    if let Some(user) = overrides.user.clone() {
        builder = builder.user(user);
    }
    Ok(builder.build()?)
}

async fn run_chat(
    client: &SimpleAgentsClient,
    model: &str,
    system: Option<&str>,
    overrides: &RequestOverrides,
    output: OutputFormat,
) -> Result<()> {
    let mut messages = Vec::new();
    if let Some(system) = system {
        messages.push(Message::system(system));
    }

    let mut stdout = io::stdout();
    let mut reader = io::BufReader::new(io::stdin());
    let mut line = String::new();

    loop {
        line.clear();
        stdout.write_all(b"user> ").await?;
        stdout.flush().await?;
        let bytes = reader.read_line(&mut line).await?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "/exit" || trimmed == "/quit" {
            break;
        }

        messages.push(Message::user(trimmed));
        let request = build_request(model, messages.clone(), overrides)?;
        let outcome = client
            .complete(&request, CompletionOptions::default())
            .await?;
        let response = match outcome {
            CompletionOutcome::Response(response) => response,
            CompletionOutcome::Stream(_) => {
                return Err(CliError::Config(
                    "streaming response returned from chat loop".to_string(),
                ))
            }
            CompletionOutcome::HealedJson(_) => {
                return Err(CliError::Config(
                    "healed json response returned from chat loop".to_string(),
                ))
            }
            CompletionOutcome::CoercedSchema(_) => {
                return Err(CliError::Config(
                    "schema response returned from chat loop".to_string(),
                ))
            }
        };
        if let Some(content) = response.content() {
            messages.push(Message::assistant(content));
        }

        print_completion(output, &response, false)?;
    }

    Ok(())
}

async fn run_benchmark(
    client: &SimpleAgentsClient,
    model: &str,
    prompt: &str,
    system: Option<&str>,
    overrides: &RequestOverrides,
    runs: u32,
    warmup: u32,
) -> Result<BenchmarkSummary> {
    for _ in 0..warmup {
        let _ = execute_completion(client, model, prompt, system, overrides).await?;
    }

    let mut durations = Vec::with_capacity(runs as usize);
    let mut total_tokens = 0u64;
    let mut token_samples = 0u64;

    for _ in 0..runs {
        let start = Instant::now();
        let response = execute_completion(client, model, prompt, system, overrides).await?;
        let elapsed = start.elapsed();
        durations.push(elapsed);
        total_tokens += response.usage.total_tokens as u64;
        token_samples += 1;
    }

    durations.sort_by_key(|duration| duration.as_millis());

    let min_ms = durations
        .first()
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let max_ms = durations
        .last()
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let avg_ms = if durations.is_empty() {
        0.0
    } else {
        durations.iter().map(|d| d.as_secs_f64()).sum::<f64>() * 1000.0 / durations.len() as f64
    };

    let p50_ms = percentile_ms(&durations, 0.50);
    let p95_ms = percentile_ms(&durations, 0.95);
    let total_tokens_avg = if token_samples == 0 {
        None
    } else {
        Some(total_tokens as f64 / token_samples as f64)
    };

    Ok(BenchmarkSummary {
        runs,
        warmup,
        avg_ms,
        min_ms,
        max_ms,
        p50_ms,
        p95_ms,
        total_tokens_avg,
    })
}

async fn run_provider_tests(
    providers: &[ProviderEntry],
    config: Option<&ConfigFile>,
    prompt: &str,
    model_override: Option<&str>,
    defaults: Option<&Defaults>,
    system: Option<&str>,
    overrides: &RequestOverrides,
) -> Result<ProviderTestReport> {
    let configured = build_provider_instances(providers)?;
    let mut results = Vec::with_capacity(configured.len());

    for provider in configured {
        let model = resolve_model(
            model_override,
            defaults,
            std::slice::from_ref(&provider.entry),
        )?;
        let client = build_single_provider_client(provider.provider.clone(), config)?;
        let start = Instant::now();
        let result = execute_completion(&client, &model, prompt, system, overrides).await;
        match result {
            Ok(_) => results.push(ProviderTestResult {
                provider: provider.entry.name().to_string(),
                success: true,
                latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
                error: None,
            }),
            Err(err) => results.push(ProviderTestResult {
                provider: provider.entry.name().to_string(),
                success: false,
                latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
                error: Some(err.to_string()),
            }),
        }
    }

    Ok(ProviderTestReport { results })
}

fn build_single_provider_client(
    provider: std::sync::Arc<dyn Provider>,
    _config: Option<&ConfigFile>,
) -> Result<SimpleAgentsClient> {
    let mut builder = SimpleAgentsClientBuilder::new().with_routing_mode(RoutingMode::Direct);
    builder = builder.with_provider(provider);
    Ok(builder.build()?)
}

fn percentile_ms(values: &[Duration], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() as f64 - 1.0) * percentile).round() as usize;
    values
        .get(index)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

fn print_completion(
    output: OutputFormat,
    response: &simple_agent_type::response::CompletionResponse,
    include_metadata: bool,
) -> Result<()> {
    match output {
        OutputFormat::Plain => {
            if include_metadata {
                let content = response.content().unwrap_or("");
                println!("{}", content);
                println!(
                    "Usage: prompt={}, completion={}, total={}",
                    response.usage.prompt_tokens,
                    response.usage.completion_tokens,
                    response.usage.total_tokens
                );
                if let Some(provider) = &response.provider {
                    println!("Provider: {}", provider);
                }
                println!("Model: {}", response.model);
            } else if let Some(content) = response.content() {
                println!("assistant> {}", content);
            }
        }
        OutputFormat::Json => {
            let value = serde_json::to_string_pretty(response)
                .map_err(|err| CliError::Serialization(err.to_string()))?;
            println!("{}", value);
        }
        OutputFormat::Markdown => {
            let content = response.content().unwrap_or("");
            println!("# Completion");
            println!("- Model: {}", response.model);
            if let Some(provider) = &response.provider {
                println!("- Provider: {}", provider);
            }
            println!(
                "- Usage: prompt={}, completion={}, total={}",
                response.usage.prompt_tokens,
                response.usage.completion_tokens,
                response.usage.total_tokens
            );
            println!("\n---\n");
            println!("{}", content);
        }
    }

    Ok(())
}

fn print_benchmark(output: OutputFormat, summary: &BenchmarkSummary) -> Result<()> {
    match output {
        OutputFormat::Plain => {
            println!("Runs: {} (warmup: {})", summary.runs, summary.warmup);
            println!("Avg: {:.2} ms", summary.avg_ms);
            println!("Min: {:.2} ms", summary.min_ms);
            println!("Max: {:.2} ms", summary.max_ms);
            println!("P50: {:.2} ms", summary.p50_ms);
            println!("P95: {:.2} ms", summary.p95_ms);
            if let Some(tokens) = summary.total_tokens_avg {
                println!("Avg tokens: {:.2}", tokens);
            }
        }
        OutputFormat::Json => {
            let value = serde_json::to_string_pretty(summary)
                .map_err(|err| CliError::Serialization(err.to_string()))?;
            println!("{}", value);
        }
        OutputFormat::Markdown => {
            println!("# Benchmark Results");
            println!("- Runs: {}", summary.runs);
            println!("- Warmup: {}", summary.warmup);
            println!("- Avg: {:.2} ms", summary.avg_ms);
            println!("- Min: {:.2} ms", summary.min_ms);
            println!("- Max: {:.2} ms", summary.max_ms);
            println!("- P50: {:.2} ms", summary.p50_ms);
            println!("- P95: {:.2} ms", summary.p95_ms);
            if let Some(tokens) = summary.total_tokens_avg {
                println!("- Avg tokens: {:.2}", tokens);
            }
        }
    }

    Ok(())
}

fn print_provider_report(output: OutputFormat, report: &ProviderTestReport) -> Result<()> {
    match output {
        OutputFormat::Plain => {
            for result in &report.results {
                if result.success {
                    println!(
                        "{}: ok ({:.2} ms)",
                        result.provider,
                        result.latency_ms.unwrap_or(0.0)
                    );
                } else {
                    println!(
                        "{}: failed ({})",
                        result.provider,
                        result.error.as_deref().unwrap_or("unknown error")
                    );
                }
            }
        }
        OutputFormat::Json => {
            let value = serde_json::to_string_pretty(report)
                .map_err(|err| CliError::Serialization(err.to_string()))?;
            println!("{}", value);
        }
        OutputFormat::Markdown => {
            println!("# Provider Health");
            for result in &report.results {
                if result.success {
                    println!(
                        "- {}: ok ({:.2} ms)",
                        result.provider,
                        result.latency_ms.unwrap_or(0.0)
                    );
                } else {
                    println!(
                        "- {}: failed ({})",
                        result.provider,
                        result.error.as_deref().unwrap_or("unknown error")
                    );
                }
            }
        }
    }

    Ok(())
}

fn run_workflow_tools(args: WorkflowArgs, output: OutputFormat) -> Result<()> {
    match args.command {
        WorkflowCommands::Trace { trace_file } => {
            let trace = read_trace_file(&trace_file)?;
            match output {
                OutputFormat::Json => {
                    let value = serde_json::to_string_pretty(&trace)
                        .map_err(|err| CliError::Serialization(err.to_string()))?;
                    println!("{}", value);
                }
                OutputFormat::Plain | OutputFormat::Markdown => {
                    println!(
                        "trace_id={} workflow={} version={}",
                        trace.metadata.trace_id,
                        trace.metadata.workflow_name,
                        trace.metadata.workflow_version
                    );
                    for event in trace.events {
                        println!("seq={} kind={:?}", event.seq, event.kind);
                    }
                }
            }
        }
        WorkflowCommands::Replay {
            trace_file,
            cache_policy,
        } => {
            let trace = read_trace_file(&trace_file)?;
            let policy = match cache_policy {
                WorkflowCachePolicyArg::Always => ReplayCachePolicy::Always,
                WorkflowCachePolicyArg::Refresh => ReplayCachePolicy::Refresh,
                WorkflowCachePolicyArg::Mixed => ReplayCachePolicy::Mixed,
            };

            let report = replay_trace_with_options(
                &trace,
                &ReplayOptions {
                    cache_policy: policy,
                },
            )
            .map_err(|err| CliError::Config(err.to_string()))?;

            match output {
                OutputFormat::Json => {
                    let value = serde_json::json!({
                        "total_events": report.total_events,
                        "terminal_status": format!("{:?}", report.terminal_status),
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value)
                            .map_err(|err| CliError::Serialization(err.to_string()))?
                    );
                }
                OutputFormat::Plain | OutputFormat::Markdown => {
                    println!(
                        "replay ok: events={} terminal={:?}",
                        report.total_events, report.terminal_status
                    );
                }
            }
        }
        WorkflowCommands::Inspect {
            trace_file,
            node_id,
        } => {
            let trace = read_trace_file(&trace_file)?;
            let inspection = inspect_replay_trace(&trace);
            let node_events: Vec<_> = trace
                .events
                .iter()
                .filter(|event| match &event.kind {
                    simple_agents_workflow::TraceEventKind::NodeEnter { node_id: id }
                    | simple_agents_workflow::TraceEventKind::NodeExit { node_id: id }
                    | simple_agents_workflow::TraceEventKind::NodeError { node_id: id, .. } => {
                        id == &node_id
                    }
                    simple_agents_workflow::TraceEventKind::Terminal { .. } => false,
                })
                .collect();

            match output {
                OutputFormat::Json => {
                    let value = serde_json::json!({
                        "valid": inspection.valid,
                        "total_events": inspection.total_events,
                        "terminal_status": inspection.terminal_status.map(|s| format!("{:?}", s)),
                        "violations": inspection.violations,
                        "node_event_count": node_events.len(),
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value)
                            .map_err(|err| CliError::Serialization(err.to_string()))?
                    );
                }
                OutputFormat::Plain | OutputFormat::Markdown => {
                    println!(
                        "inspect node={} valid={} node_events={}",
                        node_id,
                        inspection.valid,
                        node_events.len()
                    );
                    if !inspection.violations.is_empty() {
                        println!("violations:");
                        for violation in inspection.violations {
                            println!("- {}", violation);
                        }
                    }
                }
            }
        }
        WorkflowCommands::Mermaid { workflow_file } => {
            let ext = workflow_file
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            let diagram = match ext.as_str() {
                "yaml" | "yml" => yaml_workflow_file_to_mermaid(&workflow_file)
                    .map_err(|err| CliError::Config(err.to_string()))?,
                "json" => {
                    let bytes = std::fs::read(&workflow_file)?;
                    let definition = serde_json::from_slice::<WorkflowDefinition>(&bytes)
                        .map_err(|err| CliError::Config(format!("invalid workflow IR json: {}", err)))?;
                    workflow_to_mermaid(&definition)
                }
                _ => {
                    return Err(CliError::Config(
                        "unsupported workflow format (use .yaml/.yml or .json)".to_string(),
                    ))
                }
            };

            match output {
                OutputFormat::Json => {
                    let value = serde_json::json!({
                        "workflow_file": workflow_file,
                        "mermaid": diagram,
                    });
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&value)
                            .map_err(|err| CliError::Serialization(err.to_string()))?
                    );
                }
                OutputFormat::Plain | OutputFormat::Markdown => {
                    println!("{}", diagram);
                }
            }
        }
    }

    Ok(())
}

fn read_trace_file(path: &Path) -> Result<WorkflowTrace> {
    let bytes = std::fs::read(path)?;
    let trace = serde_json::from_slice::<WorkflowTrace>(&bytes)
        .map_err(|err| CliError::Config(format!("invalid trace json: {}", err)))?;
    Ok(trace)
}

impl ProviderEntry {
    fn name(&self) -> &'static str {
        match self.kind {
            ProviderKind::OpenAI => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::OpenRouter => "openrouter",
        }
    }
}
