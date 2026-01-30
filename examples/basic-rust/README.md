# Basic Rust Example

This example demonstrates core CXDB operations using the Rust client SDK.

## What It Does

1. **Connects** to CXDB binary protocol server (port 9009)
2. **Creates** a new context (conversation branch)
3. **Appends** three turns:
   - User input: "What is the weather in San Francisco?"
   - Assistant response: "Let me check the weather for you."
   - Tool call: `get_weather(location="San Francisco, CA")`
4. **Retrieves** the conversation history
5. **Displays** formatted output

## Prerequisites

- **CXDB server running** on `localhost:9009`
- **Rust 1.75+** installed

## Run It

```bash
# From this directory
cargo run
```

Or specify a custom address:

```bash
CXDB_ADDR=192.168.1.100:9009 cargo run
```

## Expected Output

```
Connecting to CXDB at localhost:9009...
Connected successfully!

Creating new context...
Created context ID: 1 (head_turn_id=0, depth=0)

Appending user turn...
Appended user turn: turn_id=1, depth=1, hash=[a3, f5, b8, c2, ...]

Appending assistant turn...
Appended assistant turn: turn_id=2, depth=2

Appending tool call turn...
Appended tool call turn: turn_id=3, depth=3

Retrieving conversation history...

Conversation history (3 turns):
======================================================================

Turn 1 (depth=1, hash=[a3, f5, b8, c2, ...]...)
  Type: com.example.Message v1
  Role: user
  Text: What is the weather in San Francisco?

Turn 2 (depth=2, hash=[b4, e6, c9, d3, ...]...)
  Type: com.example.Message v1
  Role: assistant
  Text: Let me check the weather for you.

Turn 3 (depth=3, hash=[c5, f7, d0, e4, ...]...)
  Type: com.example.ToolCall v1
  Tool: get_weather
  Arguments: {"location": "San Francisco, CA", "units": "fahrenheit"}

======================================================================

Success! View this conversation in the UI:
  http://localhost:8080/contexts/1

(Start the gateway with: cd ../../gateway && go run ./cmd/server)
```

## Key Concepts

### Connection

```rust
let client = cxdb::dial("localhost:9009", vec![])?;
```

The second parameter is for TLS configuration (empty for plain TCP).

### Serde Msgpack Tags

Use `#[serde(rename = "N")]` for numeric tags:

```rust
#[derive(Serialize, Deserialize)]
struct Message {
    #[serde(rename = "1")]
    role: String,
    #[serde(rename = "2")]
    text: String,
}
```

**Why numeric tags?**
- Forward-compatible schema evolution
- Old readers skip unknown fields
- Type registry provides JSON projection

### Context Creation

```rust
let ctx = cxdb::RequestContext::background();
let context = client.create_context(&ctx, 0)?;
// context.context_id = 1
// context.head_turn_id = 0 (empty)
```

### Appending Turns

```rust
let payload = cxdb::encode_msgpack(&my_data)?;
let turn = client.append_turn(
    &ctx,
    &cxdb::AppendRequest::new(
        context_id,
        "com.example.Message",
        1,
        payload,
    ),
)?;
```

The request context `ctx` allows for:
- Timeouts
- Cancellation
- Request metadata

### Retrieving Turns

```rust
let options = cxdb::GetLastOptions {
    limit: 10,
    include_payload: true,
};
let turns = client.get_last(&ctx, context_id, options)?;
```

Returns turns in **chronological order** (oldest first).

## Data Types

### Message

```rust
#[derive(Serialize, Deserialize)]
struct Message {
    #[serde(rename = "1")]
    role: String,
    #[serde(rename = "2")]
    text: String,
}
```

### ToolCall

```rust
#[derive(Serialize, Deserialize)]
struct ToolCall {
    #[serde(rename = "1")]
    name: String,
    #[serde(rename = "2")]
    arguments: HashMap<String, String>,
}
```

## Troubleshooting

### Connection Refused

**Error**: `Connection refused (os error 61)`

**Solution**: Start the CXDB server:
```bash
cd ../..
cargo run --release
```

### Build Errors

**Error**: `error: couldn't read Cargo.toml`

**Solution**: Ensure you're in the `examples/basic-rust/` directory:
```bash
cd examples/basic-rust
cargo build
```

### Type Not Found in UI

The web UI may show "Type not found" warnings. To enable rich visualization, publish a type registry bundle. See [../type-registration/](../type-registration/).

## Next Steps

- **[Type Registration](../type-registration/)**: Define custom types with schema descriptors
- **[Filesystem Snapshots](../fstree-snapshot/)**: Attach filesystem state to turns
- **[Agent Integration](../agent-integration/)**: Use canonical conversation types
- **[Protocol Docs](../../docs/protocol.md)**: Binary protocol details
- **[Client SDK](../../clients/rust/)**: Full Rust SDK documentation

## Production Checklist

Before using in production:

- [ ] Configure TLS in `dial()` second parameter
- [ ] Set request timeouts in `RequestContext`
- [ ] Implement retry logic with exponential backoff
- [ ] Provide idempotency keys in `AppendRequest`
- [ ] Publish type registry bundles before appending
- [ ] Enable compression for large payloads
- [ ] Use connection pooling for high concurrency

## License

Copyright 2025 StrongDM Inc
SPDX-License-Identifier: Apache-2.0
