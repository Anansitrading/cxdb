# Basic Go Example

This example demonstrates core CXDB operations using the Go client SDK.

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
- **Go 1.22+** installed

## Run It

```bash
# From this directory
go run main.go
```

## Expected Output

```
Connecting to CXDB at localhost:9009...
Connected successfully!

Creating new context...
Created context ID: 1 (head_turn_id=0, depth=0)

Appending user turn...
Appended user turn: turn_id=1, depth=1, hash=a3f5b8c2

Appending assistant turn...
Appended assistant turn: turn_id=2, depth=2

Appending tool call turn...
Appended tool call turn: turn_id=3, depth=3

Retrieving conversation history...

Conversation history (3 turns):
======================================================================

Turn 1 (depth=1, hash=a3f5b8c2...)
  Type: com.example.Message v1
  Role: user
  Text: What is the weather in San Francisco?

Turn 2 (depth=2, hash=b4e6c9d3...)
  Type: com.example.Message v1
  Role: assistant
  Text: Let me check the weather for you.

Turn 3 (depth=3, hash=c5f7d0e4...)
  Type: com.example.ToolCall v1
  Tool: get_weather
  Arguments: map[location:San Francisco, CA units:fahrenheit]

======================================================================

Success! View this conversation in the UI:
  http://localhost:8080/contexts/1

(Start the gateway with: cd ../../gateway && go run ./cmd/server)
```

## Key Concepts

### Connection

```go
client, err := cxdb.Dial("localhost:9009")
defer client.Close()
```

For production with TLS:
```go
client, err := cxdb.DialTLS("your-host:9009")
```

### Context Creation

A context is a mutable pointer to the head of a conversation branch:

```go
ctx, err := client.CreateContext(context.Background(), 0)
// ctx.ContextID = 1
// ctx.HeadTurnID = 0 (empty)
// ctx.HeadDepth = 0
```

### Msgpack Encoding

Use numeric tags for forward-compatible schemas:

```go
type Message struct {
    Role string `msgpack:"1"`  // Tag 1 = role
    Text string `msgpack:"2"`  // Tag 2 = text
}
```

**Why numeric tags?**
- Old readers ignore unknown tags
- New fields can be added without breaking old code
- Type registry maps tags → field names for JSON projection

### Appending Turns

```go
payload, _ := msgpack.Marshal(myData)
turn, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
    ContextID:   contextID,
    TypeID:      "com.example.Message",
    TypeVersion: 1,
    Payload:     payload,
})
```

The server:
1. Computes `BLAKE3(payload)`
2. Stores blob in content-addressed store (deduplicates)
3. Appends turn record to DAG
4. Updates context head pointer

### Retrieving Turns

```go
turns, err := client.GetLast(ctx, contextID, 10, true)
// Returns up to 10 most recent turns
// Last param: true = include payloads, false = metadata only
```

Turns are returned in **chronological order** (oldest first).

## Data Types

### Message

```go
type Message struct {
    Role string `msgpack:"1"`
    Text string `msgpack:"2"`
}
```

Used for user input and assistant responses.

### ToolCall

```go
type ToolCall struct {
    Name      string                 `msgpack:"1"`
    Arguments map[string]interface{} `msgpack:"2"`
}
```

Represents a function invocation request.

## Troubleshooting

### Connection Refused

**Error**: `dial tcp 127.0.0.1:9009: connection refused`

**Solution**: Start the CXDB server:
```bash
cd ../..
cargo run --release
```

### Module Errors

**Error**: `go: module github.com/strongdm/cxdb not found`

**Solution**: The `go.mod` file already has a `replace` directive pointing to the local client SDK. Run:
```bash
go mod tidy
```

### Type Not Found in UI

If you see "Type not found" errors in the web UI, you need to publish a type registry bundle. See [../type-registration/](../type-registration/) for details.

## Next Steps

- **[Type Registration](../type-registration/)**: Define custom types with type registry
- **[Filesystem Snapshots](../fstree-snapshot/)**: Attach filesystem state to turns
- **[Agent Integration](../agent-integration/)**: Use canonical conversation types
- **[Protocol Docs](../../docs/protocol.md)**: Binary protocol details
- **[Client SDK](../../clients/go/)**: Full Go SDK documentation

## Production Checklist

Before using in production:

- [ ] Use `cxdb.DialTLS()` for encrypted connections
- [ ] Set request timeouts via `cxdb.WithRequestTimeout()`
- [ ] Implement retry logic with exponential backoff
- [ ] Provide idempotency keys in `AppendRequest`
- [ ] Publish type registry bundles before appending turns
- [ ] Enable compression for payloads >1KB
- [ ] Use connection pooling for high-concurrency apps

## License

Copyright 2025 StrongDM Inc
SPDX-License-Identifier: Apache-2.0
