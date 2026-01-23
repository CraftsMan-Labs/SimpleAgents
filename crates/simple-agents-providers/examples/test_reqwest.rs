use reqwest::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let url = "http://localhost:4000/v1/chat/completions";
    let body = serde_json::json!({
        "model": "grok-code-fast-1",
        "messages": [{"role": "user", "content": "Hi"}]
    });

    println!("Sending request to: {}", url);
    println!("Body: {}", body);

    let response = client
        .post(url)
        .header("Authorization", "Bearer sk-rjHUmOzyagPP9ysGNaPGeA")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    println!("Status: {}", status);
    println!("Headers: {:?}", response.headers());

    let text = response.text().await?;
    println!("Response: {}", text);

    Ok(())
}
