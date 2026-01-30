// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! Basic CXDB example using the Rust client SDK.
//!
//! Demonstrates:
//! - Connecting to CXDB binary protocol
//! - Creating a context
//! - Appending multiple turns
//! - Retrieving conversation history

use serde::{Deserialize, Serialize};

/// Message represents a conversation message with msgpack numeric tags.
#[derive(Debug, Serialize, Deserialize)]
struct Message {
    #[serde(rename = "1")]
    role: String,
    #[serde(rename = "2")]
    text: String,
}

/// ToolCall represents a function invocation request.
#[derive(Debug, Serialize, Deserialize)]
struct ToolCall {
    #[serde(rename = "1")]
    name: String,
    #[serde(rename = "2")]
    arguments: std::collections::HashMap<String, String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Connect to CXDB
    println!("Connecting to CXDB at localhost:9009...");
    let addr = std::env::var("CXDB_ADDR").unwrap_or_else(|_| "localhost:9009".to_string());
    let client = cxdb::dial(&addr, vec![])?;
    println!("Connected successfully!");

    let ctx = cxdb::RequestContext::background();

    // Step 2: Create a context
    println!("\nCreating new context...");
    let context = client.create_context(&ctx, 0)?;
    println!(
        "Created context ID: {} (head_turn_id={}, depth={})",
        context.context_id, context.head_turn_id, context.head_depth
    );

    let context_id = context.context_id;

    // Step 3: Append a user turn
    println!("\nAppending user turn...");
    let user_msg = Message {
        role: "user".to_string(),
        text: "What is the weather in San Francisco?".to_string(),
    };
    let user_payload = cxdb::encode_msgpack(&user_msg)?;

    let user_turn = client.append_turn(
        &ctx,
        &cxdb::AppendRequest::new(
            context_id,
            "com.example.Message",
            1,
            user_payload,
        ),
    )?;
    println!(
        "Appended user turn: turn_id={}, depth={}, hash={:02x?}",
        user_turn.turn_id,
        user_turn.depth,
        &user_turn.content_hash[..8]
    );

    // Step 4: Append an assistant turn
    println!("\nAppending assistant turn...");
    let assistant_msg = Message {
        role: "assistant".to_string(),
        text: "Let me check the weather for you.".to_string(),
    };
    let assistant_payload = cxdb::encode_msgpack(&assistant_msg)?;

    let assistant_turn = client.append_turn(
        &ctx,
        &cxdb::AppendRequest::new(
            context_id,
            "com.example.Message",
            1,
            assistant_payload,
        ),
    )?;
    println!(
        "Appended assistant turn: turn_id={}, depth={}",
        assistant_turn.turn_id, assistant_turn.depth
    );

    // Step 5: Append a tool call turn
    println!("\nAppending tool call turn...");
    let mut arguments = std::collections::HashMap::new();
    arguments.insert("location".to_string(), "San Francisco, CA".to_string());
    arguments.insert("units".to_string(), "fahrenheit".to_string());

    let tool_call = ToolCall {
        name: "get_weather".to_string(),
        arguments,
    };
    let tool_payload = cxdb::encode_msgpack(&tool_call)?;

    let tool_turn = client.append_turn(
        &ctx,
        &cxdb::AppendRequest::new(
            context_id,
            "com.example.ToolCall",
            1,
            tool_payload,
        ),
    )?;
    println!(
        "Appended tool call turn: turn_id={}, depth={}",
        tool_turn.turn_id, tool_turn.depth
    );

    // Step 6: Retrieve conversation history
    println!("\nRetrieving conversation history...");
    let options = cxdb::GetLastOptions {
        limit: 10,
        include_payload: true,
    };
    let turns = client.get_last(&ctx, context_id, options)?;

    println!("\nConversation history ({} turns):", turns.len());
    println!("{}", "=".repeat(70));

    for turn in &turns {
        println!(
            "\nTurn {} (depth={}, hash={:02x?}...)",
            turn.turn_id,
            turn.depth,
            &turn.content_hash[..8]
        );
        println!("  Type: {} v{}", turn.type_id, turn.type_version);

        // Decode based on type
        match turn.type_id.as_str() {
            "com.example.Message" => {
                match cxdb::decode_msgpack::<Message>(&turn.payload) {
                    Ok(msg) => {
                        println!("  Role: {}", msg.role);
                        println!("  Text: {}", msg.text);
                    }
                    Err(e) => println!("  Error decoding: {}", e),
                }
            }
            "com.example.ToolCall" => {
                match cxdb::decode_msgpack::<ToolCall>(&turn.payload) {
                    Ok(tc) => {
                        println!("  Tool: {}", tc.name);
                        println!("  Arguments: {:?}", tc.arguments);
                    }
                    Err(e) => println!("  Error decoding: {}", e),
                }
            }
            _ => {
                println!("  Unknown type (raw bytes: {})", turn.payload.len());
            }
        }
    }

    println!("\n{}", "=".repeat(70));
    println!("\nSuccess! View this conversation in the UI:");
    println!("  http://localhost:8080/contexts/{}", context_id);
    println!("\n(Start the gateway with: cd ../../gateway && go run ./cmd/server)");

    Ok(())
}
