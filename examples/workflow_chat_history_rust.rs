use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use simple_agents_core::SimpleAgentsClient;
use simple_agents_providers::anthropic::AnthropicProvider;
use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_providers::openrouter::OpenRouterProvider;
use simple_agents_workflow::{
    run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options,
    YamlWorkflowExecutionFlags, YamlWorkflowRunOptions,
};

#[derive(Debug, Clone)]
struct Args {
    workflow: String,
    max_turns: usize,
    trace_dir: String,
    conversation_id: String,
    model: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        workflow: "workflow_email/email-chat-draft-or-clarify.yaml".to_string(),
        max_turns: 8,
        trace_dir: "examples/workflow_email/traces".to_string(),
        conversation_id: String::new(),
        model: None,
    };

    let argv: Vec<String> = env::args().collect();
    let mut i = 1usize;
    while i < argv.len() {
        match argv[i].as_str() {
            "--workflow" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--workflow requires a value".to_string());
                }
                args.workflow = argv[i].clone();
            }
            "--max-turns" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--max-turns requires a value".to_string());
                }
                args.max_turns = argv[i]
                    .parse::<usize>()
                    .map_err(|_| "--max-turns must be a positive integer".to_string())?;
            }
            "--trace-dir" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--trace-dir requires a value".to_string());
                }
                args.trace_dir = argv[i].clone();
            }
            "--conversation-id" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--conversation-id requires a value".to_string());
                }
                args.conversation_id = argv[i].clone();
            }
            "--model" => {
                i += 1;
                if i >= argv.len() {
                    return Err("--model requires a value".to_string());
                }
                args.model = Some(argv[i].clone());
            }
            "--help" | "-h" => {
                println!(
                    "Usage: workflow_chat_history_rust [--workflow PATH] [--max-turns N] [--trace-dir DIR] [--conversation-id ID] [--model MODEL]"
                );
                std::process::exit(0);
            }
            other => {
                return Err(format!("unrecognized argument: {}", other));
            }
        }
        i += 1;
    }

    if args.max_turns == 0 {
        return Err("--max-turns must be greater than 0".to_string());
    }

    if args.conversation_id.trim().is_empty() {
        args.conversation_id = new_session_id();
    }

    Ok(args)
}

fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:032x}", nanos)
}

fn load_env() {
    let _ = dotenv::from_path("examples/.env");
    let _ = dotenv::from_path(".env");
    dotenv::dotenv().ok();
}

fn env_or(primary: &str, fallback: &str) -> Option<String> {
    env::var(primary)
        .ok()
        .or_else(|| env::var(fallback).ok())
        .filter(|value| !value.trim().is_empty())
}

fn resolve_workflow_path(workflow: &str) -> Result<PathBuf, String> {
    let direct = PathBuf::from(workflow);
    if direct.exists() {
        return Ok(direct);
    }

    let prefixed = PathBuf::from("examples").join(workflow);
    if prefixed.exists() {
        return Ok(prefixed);
    }

    if let Some(trimmed) = workflow.strip_prefix("examples/") {
        let trimmed_path = PathBuf::from("examples").join(trimmed);
        if trimmed_path.exists() {
            return Ok(trimmed_path);
        }
    }

    Err(format!("workflow file not found: {}", workflow))
}

fn build_client() -> Result<SimpleAgentsClient, Box<dyn std::error::Error>> {
    load_env();

    let provider = env::var("WORKFLOW_PROVIDER")
        .unwrap_or_else(|_| "openai".to_string())
        .to_lowercase();
    let api_base = env_or("WORKFLOW_API_BASE", "CUSTOM_API_BASE");
    let api_key = env_or("WORKFLOW_API_KEY", "CUSTOM_API_KEY")
        .ok_or("Set WORKFLOW_API_KEY (or CUSTOM_API_KEY)")?;

    match provider.as_str() {
        "openai" => {
            env::set_var("OPENAI_API_KEY", api_key);
            if let Some(base) = api_base {
                env::set_var("OPENAI_API_BASE", base);
            }
            let provider = OpenAIProvider::from_env()?;
            Ok(SimpleAgentsClient::builder()
                .with_provider(Arc::new(provider))
                .build()?)
        }
        "anthropic" => {
            env::set_var("ANTHROPIC_API_KEY", api_key);
            let provider = AnthropicProvider::from_env()?;
            Ok(SimpleAgentsClient::builder()
                .with_provider(Arc::new(provider))
                .build()?)
        }
        "openrouter" => {
            env::set_var("OPENROUTER_API_KEY", api_key);
            if let Some(base) = api_base {
                env::set_var("OPENROUTER_API_BASE", base);
            }
            let provider = OpenRouterProvider::from_env()?;
            Ok(SimpleAgentsClient::builder()
                .with_provider(Arc::new(provider))
                .build()?)
        }
        _ => Err(format!("Unsupported WORKFLOW_PROVIDER: {}", provider).into()),
    }
}

fn render_output(value: &Option<Value>) -> String {
    let Some(value) = value.as_ref() else {
        return String::new();
    };
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    serde_json::to_string_pretty(value).unwrap_or_default()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args =
        parse_args().map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    let workflow_path = resolve_workflow_path(&args.workflow)
        .map_err(|message| io::Error::new(io::ErrorKind::NotFound, message))?;

    let trace_dir = PathBuf::from(&args.trace_dir);
    fs::create_dir_all(&trace_dir)?;

    let session_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let trace_file = trace_dir.join(format!(
        "chat-session-{}-{}.jsonl",
        session_timestamp, args.conversation_id
    ));

    let client = build_client()?;

    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": "You are a friendly email drafting assistant for new users. First, explain capabilities clearly when asked what you can do. Then gather missing scenario details and draft concise professional emails. If context is incomplete, ask one specific follow-up question."
    })];

    println!("Chat Email Assistant");
    println!("Type your request. Type 'exit' to quit.\n");
    println!("Conversation ID: {}", args.conversation_id);
    println!("Trace log: {}\n", trace_file.display());

    for turn in 1..=args.max_turns {
        print!("You: ");
        io::stdout().flush()?;

        let mut user_input = String::new();
        if io::stdin().read_line(&mut user_input)? == 0 {
            println!("Bye!");
            return Ok(());
        }
        let user_input = user_input.trim().to_string();
        if user_input.is_empty() {
            continue;
        }

        let lowered = user_input.to_lowercase();
        if lowered == "exit" || lowered == "quit" {
            println!("Bye!");
            return Ok(());
        }

        messages.push(json!({"role": "user", "content": user_input.clone()}));

        let workflow_input = json!({
            "email_text": user_input,
            "messages": messages,
        });

        let mut options = YamlWorkflowRunOptions::default();
        options.trace.tenant.conversation_id = Some(args.conversation_id.clone());
        if let Some(model) = args.model.as_ref() {
            let trimmed = model.trim();
            if !trimmed.is_empty() {
                options.model = Some(trimmed.to_string());
            }
        }

        let result = run_workflow_yaml_file_with_client_and_custom_worker_and_events_and_options(
            Path::new(&workflow_path),
            &workflow_input,
            &client,
            None,
            None,
            &options,
            YamlWorkflowExecutionFlags::default(),
        )
        .await?;

        let reply = render_output(&result.terminal_output);
        println!("\nAssistant: {}\n", reply);

        messages.push(json!({"role": "assistant", "content": reply}));

        let trace_record = json!({
            "turn": turn,
            "conversation_id": args.conversation_id,
            "workflow_path": workflow_path,
            "workflow_id": result.workflow_id,
            "terminal_node": result.terminal_node,
            "trace": result.trace,
            "step_timings": result.step_timings,
            "total_elapsed_ms": result.total_elapsed_ms,
            "trace_id": result.trace_id,
            "assistant_output": result.terminal_output,
        });
        let mut handle = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&trace_file)?;
        writeln!(handle, "{}", serde_json::to_string(&trace_record)?)?;
    }

    println!("Reached max turns. Restart to continue.");
    Ok(())
}
