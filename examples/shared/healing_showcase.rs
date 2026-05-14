use futures_util::StreamExt;
use simple_agent_type::prelude::*;
use simple_agents_healing::prelude::*;
use simple_agents_providers::openai::OpenAiCompatProvider;
use simple_agents_providers::Provider;
use std::io::Write;

pub async fn example_streaming_healing(
    provider: &OpenAiCompatProvider,
    model: &str,
    metric_source: &str,
    metric_heading: &str,
) -> Result<()> {
    let _ = metric_source;
    println!("\n📤 Streaming JSON response with healing (Large JSON)...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON. Wrap JSON in markdown.",
        ))
        .message(Message::user(
            "Create a comprehensive JSON object for a senior software engineer named Charlie \
             with these fields: name, age, email, phone, address (street, city, state, zip, country), \
             skills (array of at least 5 skills), experience (array of job objects with title, company, years), \
             education (array with degree, school, year), projects (array of 3 project objects with name, description, tech_stack, url), \
             languages (array with language, proficiency), certifications (array), available (boolean), hourly_rate, github, linkedin.",
        ))
        .temperature(0.7)
        .max_tokens(500)
        .stream(true)
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    println!("📝 Streaming with progressive healing:");
    println!("{}", "━".repeat(60));

    let mut full_content = String::new();
    let mut streaming_parser = StreamingParser::new();
    let mut chunk_count = 0;
    let mut heal_count = 0;
    let mut last_parse_size = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                        full_content.push_str(content);

                        streaming_parser.feed(content);

                        if let Some(parse_result) = streaming_parser.try_parse().ok().flatten() {
                            if !parse_result.flags.is_empty() {
                                heal_count = parse_result.flags.len();
                            }

                            let current_size = serde_json::to_string(&parse_result.value)
                                .unwrap_or_default()
                                .len();
                            if current_size > last_parse_size + 500
                                || parse_result.value.get("projects").is_some()
                                    && last_parse_size == 0
                            {
                                last_parse_size = current_size;
                                println!(
                                    "\n\n🔍 Progressive parse ({} bytes, {:.2}% complete):",
                                    current_size,
                                    (full_content.len() as f32 / 500.0 * 100.0).min(100.0)
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("\n❌ Stream error: {}", e);
                break;
            }
        }
    }

    println!("\n{}", "━".repeat(60));

    let final_result = streaming_parser.finalize()?;

    println!("\n✅ Final Healed Result:");
    println!("  Confidence: {:.2}", final_result.confidence);
    println!(
        "  Total fields: {}",
        final_result.value.as_object().map(|o| o.len()).unwrap_or(0)
    );
    println!(
        "  JSON size: {} bytes",
        serde_json::to_string(&final_result.value)
            .unwrap_or_default()
            .len()
    );

    if let Some(obj) = final_result.value.as_object() {
        if let Some(skills) = obj.get("skills").and_then(|v| v.as_array()) {
            println!("  Skills: {} items", skills.len());
        }
        if let Some(experience) = obj.get("experience").and_then(|v| v.as_array()) {
            println!("  Experience: {} items", experience.len());
        }
        if let Some(projects) = obj.get("projects").and_then(|v| v.as_array()) {
            println!("  Projects: {} items", projects.len());
        }
        if let Some(education) = obj.get("education").and_then(|v| v.as_array()) {
            println!("  Education: {} items", education.len());
        }
    }

    if !final_result.flags.is_empty() {
        println!("\n  🔧 Healing transformations:");
        for flag in &final_result.flags {
            println!("    - {}", flag.description());
        }
    }

    println!("\n{}:", metric_heading);
    println!("  Chunks: {}", chunk_count);
    println!("  Healing operations: {}", heal_count);
    println!("  Total length: {} characters", full_content.len());

    Ok(())
}

pub async fn example_streaming_structured(
    provider: &OpenAiCompatProvider,
    model: &str,
    metric_source: &str,
    metric_heading: &str,
) -> Result<()> {
    let _ = metric_source;
    println!("\n📤 Streaming structured JSON with progressive parsing (Large Array)...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON array of 8 products with id, name, price, in_stock, category, tags (array), rating, and description fields.",
        ))
        .temperature(0.5)
        .max_tokens(500)
        .stream(true)
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    println!("📝 Progressive structured output:");
    println!("{}", "━".repeat(60));

    let mut full_content = String::new();
    let mut streaming_parser = StreamingParser::new();
    let mut chunk_count = 0;
    let mut partial_count = 0;
    let mut last_item_count = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                        full_content.push_str(content);

                        streaming_parser.feed(content);

                        if let Some(parse_result) = streaming_parser.try_parse().ok().flatten() {
                            partial_count += 1;

                            let current_items =
                                parse_result.value.as_array().map(|a| a.len()).unwrap_or(0);

                            if partial_count == 1 || current_items > last_item_count {
                                last_item_count = current_items;
                                println!(
                                    "\n\n🔍 Progressive parse #{} ({} items, {:.2}% complete):",
                                    partial_count,
                                    current_items,
                                    (full_content.len() as f32 / 500.0 * 100.0).min(100.0)
                                );

                                if let Some(arr) = parse_result.value.as_array() {
                                    let start = if arr.len() > 3 { arr.len() - 3 } else { 0 };
                                    for (i, item) in arr.iter().enumerate().skip(start) {
                                        println!(
                                            "  [{}] {}",
                                            i,
                                            serde_json::to_string(item).unwrap_or_default()
                                        );
                                    }
                                }
                                println!("{}", "━".repeat(60));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("\n❌ Stream error: {}", e);
                break;
            }
        }
    }

    println!("\n{}", "━".repeat(60));

    let final_result = streaming_parser.finalize()?;

    println!("\n✅ Final Structured Output:");
    println!("  Confidence: {:.2}", final_result.confidence);
    println!(
        "  Total items in array: {}",
        final_result.value.as_array().map(|a| a.len()).unwrap_or(0)
    );
    println!(
        "  JSON size: {} bytes",
        serde_json::to_string(&final_result.value)
            .unwrap_or_default()
            .len()
    );

    if !final_result.flags.is_empty() {
        println!("\n  🔧 Healing applied:");
        for flag in &final_result.flags {
            println!("    - {}", flag.description());
        }
    }

    if let Some(arr) = final_result.value.as_array() {
        if arr.len() > 4 {
            println!("\n  Sample items:");
            for i in [0, 1, arr.len() - 2, arr.len() - 1] {
                println!("    [{}]", i);
                for (k, v) in arr[i].as_object().unwrap_or(&serde_json::Map::new()) {
                    println!("      {}: {}", k, v);
                }
            }
        }
    }

    println!("\n{}:", metric_heading);
    println!("  Chunks: {}", chunk_count);
    println!("  Partial parses: {}", partial_count);
    println!("  Total length: {} characters", full_content.len());

    Ok(())
}

pub async fn example_streaming_graph(
    provider: &OpenAiCompatProvider,
    model: &str,
    metric_source: &str,
    metric_heading: &str,
) -> Result<()> {
    let _ = metric_source;
    println!("\n📤 Streaming graph data with progressive visualization...\n");

    let request = CompletionRequest::builder()
        .model(model)
        .message(Message::system(
            "You are a helpful assistant. Always respond with JSON.",
        ))
        .message(Message::user(
            "Create a JSON graph representing a software architecture with these fields: \
             nodes (array of objects with id, name, type, group) - include at least 10 nodes \
             representing services, databases, queues, and frontend; \
             edges (array of objects with source, target, type, label) - create connections between nodes; \
             layout (object with type: 'hierarchical', direction: 'top-down').",
        ))
        .temperature(0.6)
        .max_tokens(600)
        .stream(true)
        .build()?;

    let provider_request = provider.transform_request(&request)?;
    let mut stream = provider.execute_stream(provider_request).await?;

    println!("📝 Progressive graph visualization:");
    println!("{}", "━".repeat(60));

    let mut full_content = String::new();
    let mut streaming_parser = StreamingParser::new();
    let mut chunk_count = 0;
    let mut partial_count = 0;
    let mut last_node_count = 0;
    let mut last_edge_count = 0;

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                chunk_count += 1;

                if let Some(choice) = chunk.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        print!("{}", content);
                        std::io::stdout().flush().unwrap();
                        full_content.push_str(content);

                        streaming_parser.feed(content);

                        if let Some(parse_result) = streaming_parser.try_parse().ok().flatten() {
                            partial_count += 1;

                            let nodes = parse_result
                                .value
                                .get("nodes")
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);

                            let edges = parse_result
                                .value
                                .get("edges")
                                .and_then(|v| v.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);

                            if nodes > last_node_count || edges > last_edge_count {
                                last_node_count = nodes;
                                last_edge_count = edges;

                                println!("\n\n🔍 Progressive graph update #{}", partial_count);
                                println!("  📊 Nodes: {} | Edges: {}", nodes, edges);
                                println!(
                                    "  📈 Progress: {:.1}%",
                                    (full_content.len() as f32 / 600.0 * 100.0).min(100.0)
                                );
                                println!("  🔧 Confidence: {:.2}", parse_result.confidence);

                                if let Some(node_arr) =
                                    parse_result.value.get("nodes").and_then(|v| v.as_array())
                                {
                                    println!("\n  🎨 Live Graph Preview:");
                                    println!("  {}", "─".repeat(50));

                                    let mut groups: std::collections::HashMap<&str, Vec<&str>> =
                                        std::collections::HashMap::new();
                                    for node in node_arr.iter() {
                                        if let Some(name) =
                                            node.get("name").and_then(|v| v.as_str())
                                        {
                                            if let Some(typ) =
                                                node.get("type").and_then(|v| v.as_str())
                                            {
                                                groups.entry(typ).or_default().push(name);
                                            }
                                        }
                                    }

                                    for (typ, names) in groups.iter() {
                                        let icon = match *typ {
                                            "service" => "⚙️",
                                            "database" => "🗄️",
                                            "queue" => "📨",
                                            "frontend" => "🖥️",
                                            _ => "📦",
                                        };
                                        println!("  {} [{}] {}:", icon, typ, names.join(", "));
                                    }

                                    println!("  {}", "─".repeat(50));
                                }
                                println!("{}", "━".repeat(60));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("\n❌ Stream error: {}", e);
                break;
            }
        }
    }

    println!("\n{}", "━".repeat(60));

    let final_result = streaming_parser.finalize()?;

    println!("\n✅ Final Graph Structure:");
    println!("  Confidence: {:.2}", final_result.confidence);
    println!(
        "  JSON size: {} bytes",
        serde_json::to_string(&final_result.value)
            .unwrap_or_default()
            .len()
    );

    if let Some(nodes) = final_result.value.get("nodes").and_then(|v| v.as_array()) {
        println!("  📊 Total nodes: {}", nodes.len());

        let mut type_counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for node in nodes.iter() {
            if let Some(typ) = node.get("type").and_then(|v| v.as_str()) {
                *type_counts.entry(typ).or_insert(0) += 1;
            }
        }

        println!("\n  🎯 Node Type Distribution:");
        for (typ, count) in type_counts {
            let bar = "█".repeat(count * 2);
            println!("    {} [{}]: {}", typ, count, bar);
        }

        println!("\n  📝 Node Details:");
        for (i, node) in nodes.iter().enumerate().take(5) {
            if let Some(obj) = node.as_object() {
                let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                let group = obj.get("group").and_then(|v| v.as_str()).unwrap_or("-");
                println!("    [{}] {} | {} | {} | group: {}", i, id, name, typ, group);
            }
        }
        if nodes.len() > 5 {
            println!("    ... and {} more nodes", nodes.len() - 5);
        }
    }

    if let Some(edges) = final_result.value.get("edges").and_then(|v| v.as_array()) {
        println!("\n  🔗 Total edges: {}", edges.len());

        println!("\n  🔗 Edge Sample:");
        for (i, edge) in edges.iter().enumerate().take(3) {
            if let Some(obj) = edge.as_object() {
                let source = obj.get("source").and_then(|v| v.as_str()).unwrap_or("?");
                let target = obj.get("target").and_then(|v| v.as_str()).unwrap_or("?");
                let typ = obj.get("type").and_then(|v| v.as_str()).unwrap_or("-");
                let label = obj.get("label").and_then(|v| v.as_str()).unwrap_or("");
                println!(
                    "    [{}] {} --> {} | type: {} | label: '{}'",
                    i, source, target, typ, label
                );
            }
        }
        if edges.len() > 3 {
            println!("    ... and {} more edges", edges.len() - 3);
        }
    }

    if let Some(layout) = final_result.value.get("layout").and_then(|v| v.as_object()) {
        println!("\n  📐 Layout Config:");
        for (k, v) in layout {
            println!("    {}: {}", k, v);
        }
    }

    if !final_result.flags.is_empty() {
        println!("\n  🔧 Healing applied:");
        for flag in &final_result.flags {
            println!("    - {}", flag.description());
        }
    }

    println!("\n{}:", metric_heading);
    println!("  Chunks: {}", chunk_count);
    println!("  Progressive updates: {}", partial_count);
    println!("  Total length: {} characters", full_content.len());

    Ok(())
}
