use simple_agents_providers::openai::OpenAIProvider;
use simple_agents_types::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== Testing API Call to localhost:4000 ===\n");

    // Create API key
    let api_key = ApiKey::new("sk-FmvNVRt1xixqgbC5kDx4YQ")?;
    println!("✓ API Key created: {}", api_key.preview());

    // Create provider with custom base URL
    let provider = OpenAIProvider::with_base_url(
        api_key,
        "http://localhost:4000".to_string()
    )?;
    println!("✓ Provider created with base URL: {}\n", provider.base_url());

    // Build request
    let request = CompletionRequest::builder()
        .model("grok-code-fast-1")
        .message(Message::system("You are a helpful assistant."))
        .message(Message::user("Hello! What is 2+2?"))
        .temperature(0.7)
        .max_tokens(100)
        .build()?;

    println!("✓ Request built:");
    println!("  Model: {}", request.model);
    println!("  Messages: {}", request.messages.len());
    println!("  Temperature: {:?}", request.temperature);
    println!("  Max tokens: {:?}\n", request.max_tokens);

    // Transform request
    println!("→ Transforming request...");
    let provider_request = provider.transform_request(&request)?;
    println!("✓ Request transformed");
    println!("  URL: {}", provider_request.url);
    println!("  Headers: {} headers set\n", provider_request.headers.len());

    // Execute request
    println!("→ Sending request to API...");
    match provider.execute(provider_request).await {
        Ok(provider_response) => {
            println!("✓ Response received!");
            println!("  Status: {}", provider_response.status);
            println!("  Body:\n{}\n", serde_json::to_string_pretty(&provider_response.body)?);

            // Transform response
            println!("→ Transforming response...");
            let response = provider.transform_response(provider_response)?;

            println!("✓ Response transformed!");
            println!("\n=== Final Response ===");
            println!("ID: {}", response.id);
            println!("Model: {}", response.model);
            println!("Content: {}", response.content().unwrap_or("No content"));
            println!("Usage: {} prompt + {} completion = {} total tokens",
                     response.usage.prompt_tokens,
                     response.usage.completion_tokens,
                     response.usage.total_tokens);
            println!("\n=== Success! ===");
        }
        Err(e) => {
            println!("✗ Error: {}", e);
            println!("\nMake sure your API server is running on localhost:4000");
        }
    }

    Ok(())
}
