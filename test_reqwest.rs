use reqwest::Client;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .http1_only()
        .build()?;

    println!("Attempting to connect to router.requesty.ai...");
    
    let response = client
        .post("https://router.requesty.ai/v1/chat/completions")
        .header("Authorization", "Bearer rqsty-sk-Te6INRaBRGG3f0RUHFNd03JWD9uPIRl6s1xtk/n8ic4BOJfhR5L00vcVKsvsXb+FG4BFBnWQQT78y+zci5HtRs8o3jPxSQ49wpGO7nkPGgo=")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "azure/gpt-4.1",
            "messages": [{"role": "user", "content": "Say hello"}],
            "max_tokens": 50
        }))
        .send()
        .await;

    match response {
        Ok(resp) => {
            println!("Success! Status: {}", resp.status());
            let text = resp.text().await?;
            println!("Response: {}", text);
        }
        Err(e) => {
            println!("Error: {:?}", e);
            println!("Is timeout: {}", e.is_timeout());
            println!("Is connect: {}", e.is_connect());
            println!("Is request: {}", e.is_request());
        }
    }

    Ok(())
}
