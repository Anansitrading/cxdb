# Agent Integration Example

This example demonstrates integrating AI agent frameworks with CXDB using the HTTP API and canonical conversation types.

## What It Does

1. **Creates** a context via HTTP API
2. **Simulates** a multi-turn agent conversation:
   - User input: "What's the weather in San Francisco?"
   - Assistant + tool call: `get_weather()`
   - Assistant response with tool results
   - Follow-up user question
   - Follow-up assistant response
   - System message with token usage
3. **Retrieves** and displays the formatted conversation
4. **Uses** canonical `cxdb.ConversationItem` types for rich UI rendering

## Prerequisites

- **CXDB server running** on `localhost:9010` (HTTP API)
- **Python 3.9+** installed

## Run It

```bash
# Install dependencies
pip install -r requirements.txt

# Run the agent
python agent.py
```

Or make it executable:

```bash
chmod +x agent.py
./agent.py
```

## Expected Output

```
AI Agent Integration Example
======================================================================

Creating context...
Created context 1

Simulating agent conversation in context 1...

[Turn 1] User input
  Appended turn 1
[Turn 2] Assistant + tool call
  Appended turn 2
[Turn 3] Assistant response
  Appended turn 3
[Turn 4] User follow-up
  Appended turn 4
[Turn 5] Assistant response
  Appended turn 5
[Turn 6] System info
  Appended turn 6

======================================================================
Conversation (Context 1)
======================================================================

[Turn 1] USER_INPUT @ 10:30:45
  User: What's the weather like in San Francisco?

[Turn 2] ASSISTANT_TURN @ 10:30:46
  Assistant: Let me check the current weather for you.

  Tool calls:
    - get_weather({"location": "San Francisco, CA", "units": "fahrenheit"})
      → {"temperature": 62, "conditions": "partly cloudy", "humidity": 65, "wind_speed": 12}

[Turn 3] ASSISTANT_TURN @ 10:30:47
  Assistant: The weather in San Francisco is currently 62°F and partly cloudy.
  Humidity is at 65% with winds at 12 mph. It's a pleasant day!

[Turn 4] USER_INPUT @ 10:30:48
  User: Should I bring a jacket?

[Turn 5] ASSISTANT_TURN @ 10:30:49
  Assistant: Yes, I'd recommend bringing a light jacket. While 62°F is mild,
  San Francisco can feel cooler with the wind and fog, especially near the coast.

[Turn 6] SYSTEM @ 10:30:50
  [INFO] Conversation token usage: 450 tokens (350 input, 100 output)

======================================================================

Success! View the conversation in the UI:
  http://localhost:9010/contexts/1

The UI provides:
  - Rich conversation visualization
  - Tool call inspection
  - Timeline view
  - Turn-by-turn navigation

(Start the gateway with: cd ../../gateway && go run ./cmd/server)
```

## Key Concepts

### HTTP API

This example uses CXDB's HTTP API instead of the binary protocol:

```python
import requests

# Create context
resp = requests.post("http://localhost:9010/v1/contexts/create")
context_id = resp.json()["context_id"]

# Append turn
resp = requests.post(
    f"http://localhost:9010/v1/contexts/{context_id}/append",
    json={
        "type_id": "cxdb.ConversationItem",
        "type_version": 3,
        "data": conversation_item,
    }
)

# Get turns
resp = requests.get(f"http://localhost:9010/v1/contexts/{context_id}/turns?limit=10")
turns = resp.json()["turns"]
```

**Benefits:**
- Simple HTTP/JSON (no msgpack encoding)
- Works from any language
- Server handles type projection

**Tradeoffs:**
- Slightly higher latency vs binary protocol
- Less efficient for high-throughput scenarios
- No compression or connection multiplexing

### Canonical ConversationItem Types

CXDB provides well-defined conversation types that the UI renders richly:

```python
TYPE_ID = "cxdb.ConversationItem"
TYPE_VERSION = 3
```

**Item types:**
- `user_input`: User messages
- `assistant_turn`: Assistant responses with nested tool calls
- `system`: System messages (info/warning/error)
- `handoff`: Agent-to-agent handoffs

**Why use canonical types?**
- Rich UI visualization (avatars, formatting, tool inspection)
- Type safety across agent frameworks
- Consistent logging format
- Built-in type registry (no bundle needed)

### User Input

```python
{
    "item_type": "user_input",
    "timestamp": 1706615000000,  # unix milliseconds
    "text": "What's the weather in San Francisco?",
    "attachments": []
}
```

### Assistant Turn with Tool Calls

```python
{
    "item_type": "assistant_turn",
    "timestamp": 1706615001000,
    "text": "Let me check the weather for you.",
    "tool_calls": [
        {
            "call_id": "call_001",
            "name": "get_weather",
            "arguments": {"location": "San Francisco, CA"},
            "status": "complete",
            "result": {"temperature": 62, "conditions": "partly cloudy"}
        }
    ],
    "status": "complete"
}
```

**Tool call statuses:**
- `pending`: Queued
- `executing`: Running
- `complete`: Success
- `error`: Failed
- `cancelled`: Aborted

### System Messages

```python
{
    "item_type": "system",
    "timestamp": 1706615002000,
    "text": "Conversation token usage: 450 tokens",
    "severity": "info"  # info, warning, error, critical
}
```

## Integration Patterns

### LangChain Integration

```python
from langchain.callbacks.base import BaseCallbackHandler

class CXDBLogger(BaseCallbackHandler):
    def __init__(self, context_id):
        self.context_id = context_id

    def on_llm_start(self, serialized, prompts, **kwargs):
        # Log user input
        append_turn(self.context_id, user_input(prompts[0]))

    def on_tool_start(self, serialized, input_str, **kwargs):
        # Log tool call
        tool = {
            "call_id": kwargs.get("run_id"),
            "name": serialized["name"],
            "arguments": {"input": input_str},
            "status": "executing",
        }
        # Update assistant turn with tool call

    def on_llm_end(self, response, **kwargs):
        # Log assistant response
        append_turn(
            self.context_id,
            assistant_turn(response.generations[0][0].text)
        )
```

### OpenAI SDK Integration

```python
import openai

def log_openai_conversation(context_id, messages):
    for msg in messages:
        if msg["role"] == "user":
            append_turn(context_id, user_input(msg["content"]))
        elif msg["role"] == "assistant":
            tool_calls = []
            if msg.get("tool_calls"):
                for tc in msg["tool_calls"]:
                    tool_calls.append({
                        "call_id": tc["id"],
                        "name": tc["function"]["name"],
                        "arguments": json.loads(tc["function"]["arguments"]),
                        "status": "complete",
                    })
            append_turn(
                context_id,
                assistant_turn(msg["content"], tool_calls=tool_calls)
            )
```

### Custom Agent Framework

```python
class MyAgent:
    def __init__(self):
        self.context_id = create_context()

    def process_user_message(self, text):
        # Log user input
        append_turn(self.context_id, user_input(text))

        # Generate response
        response = self.generate_response(text)

        # Log assistant response
        append_turn(self.context_id, assistant_turn(response))

        return response

    def call_tool(self, name, args):
        # Execute tool
        result = self.tools[name](**args)

        # Log tool call in assistant turn
        tool_call = {
            "call_id": str(uuid.uuid4()),
            "name": name,
            "arguments": args,
            "status": "complete",
            "result": result,
        }

        append_turn(
            self.context_id,
            assistant_turn("", tool_calls=[tool_call])
        )

        return result
```

## Configuration

Edit `config.toml` to customize:

```toml
[cxdb]
http_url = "http://localhost:9010"

[agent]
name = "my-agent"
version = "1.0.0"
max_retries = 3
timeout_seconds = 30

[logging]
level = "info"
verbose = true
```

## Advanced Features

### Provenance Metadata

Track conversation origins:

```python
provenance = {
    "agent_id": "my-agent",
    "agent_version": "1.0.0",
    "user_id": "user123",
    "session_id": "sess456",
    "environment": "production",
}

# Add to first turn or context creation
```

### Attachments

Attach files to user input:

```python
user_input(
    text="Analyze this image",
    attachments=[
        {
            "name": "photo.jpg",
            "mime_type": "image/jpeg",
            "bytes": base64_encoded_data,
        }
    ]
)
```

### Error Handling

Handle failed tool calls:

```python
tool_call = {
    "call_id": "call_001",
    "name": "database_query",
    "arguments": {"query": "SELECT *"},
    "status": "error",
    "error": {
        "code": "TIMEOUT",
        "message": "Database connection timeout after 30s",
    }
}
```

### Streaming Responses

Log partial responses:

```python
# Start with streaming status
append_turn(
    context_id,
    assistant_turn("", status="streaming")
)

# Update as chunks arrive
for chunk in stream:
    accumulated_text += chunk

# Final complete turn
append_turn(
    context_id,
    assistant_turn(accumulated_text, status="complete")
)
```

## Troubleshooting

### Connection Refused

**Error**: `requests.exceptions.ConnectionError`

**Solution**: Ensure CXDB server is running:
```bash
cd ../..
cargo run --release
```

The HTTP API runs on the same process as the binary protocol (port 9010).

### Type Not Found

**Error**: `424 Failed Dependency: type not found`

**Solution**: Canonical types (`cxdb.ConversationItem`) are built into the server. If you see this error, ensure:
1. Server is up to date
2. Using correct type_id: `"cxdb.ConversationItem"`
3. Using correct version: `3`

### Invalid Item Type

**Error**: `400 Bad Request: invalid item_type`

**Solution**: Valid item types are:
- `user_input`
- `assistant_turn`
- `system`
- `handoff`

### JSON Encoding Error

**Error**: `TypeError: Object of type bytes is not JSON serializable`

**Solution**: Base64-encode binary data before sending:
```python
import base64
encoded = base64.b64encode(binary_data).decode('ascii')
```

## Next Steps

- **[Basic Go](../basic-go/)**: Learn binary protocol for high-throughput
- **[Type Registration](../type-registration/)**: Define custom types
- **[HTTP API Docs](../../docs/http-api.md)**: Complete API reference
- **[Conversation Types](../../clients/go/types/)**: Canonical type definitions

## Best Practices

1. **Log all turns**: User inputs, assistant responses, tool calls, errors
2. **Use timestamps**: Include millisecond-precision timestamps
3. **Provide tool results**: Embed results in tool_calls for inspection
4. **Handle errors**: Log failed tool calls with error details
5. **Add provenance**: Track agent/user/session info
6. **Use system messages**: Log metadata (tokens, timing, guardrails)
7. **Test error cases**: Ensure your integration handles API failures

## License

Copyright 2025 StrongDM Inc
SPDX-License-Identifier: Apache-2.0
