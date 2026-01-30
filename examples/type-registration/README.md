# Type Registration Example

This example demonstrates CXDB's type registry for forward-compatible schema evolution.

## What It Does

1. **Defines** a custom `LogEntry` type with msgpack numeric tags
2. **Creates** a type registry bundle (JSON descriptor)
3. **Publishes** the bundle to the CXDB server via HTTP API
4. **Appends** log entry turns with the custom type
5. **Retrieves** and displays the logs
6. **Demonstrates** typed JSON projection in the web UI

## Prerequisites

- **CXDB server running** on `localhost:9009` (binary) and `localhost:9010` (HTTP)
- **Go 1.22+** installed

## Run It

```bash
# From this directory
go run *.go
```

## Expected Output

```
Reading type registry bundle from bundle.json...
Bundle loaded successfully

Publishing type registry bundle to server...
Bundle published successfully (HTTP 200)

Connecting to CXDB at localhost:9009...
Connected successfully!

Creating new context...
Created context ID: 1

Appending log entries...
  [1] INFO: Application started (turn_id=1)
  [2] WARN: High memory usage detected (turn_id=2)
  [3] ERROR: Failed to connect to database (turn_id=3)
  [4] DEBUG: Cache hit for user profile (turn_id=4)

Retrieving log entries...

Retrieved 4 log entries:
=========================================================================

[Turn 1] INFO - Application started
  Timestamp: 1706615000000 (unix_ms)
  Tags:
    env: production
    version: 1.0.0

[Turn 2] WARN - High memory usage detected
  Timestamp: 1706615001000 (unix_ms)
  Tags:
    limit_mb: 4096
    usage_mb: 2048

[Turn 3] ERROR - Failed to connect to database
  Timestamp: 1706615002000 (unix_ms)
  Tags:
    error: connection timeout
    host: db.example.com

[Turn 4] DEBUG - Cache hit for user profile
  Timestamp: 1706615003000 (unix_ms)
  Tags:
    key: profile:12345
    user_id: 12345

=========================================================================

Success! View the typed JSON projection in the UI:
  http://localhost:9010/contexts/1/turns?view=typed

The type registry enables:
  - Numeric tags → field names (e.g., 1 → 'timestamp')
  - Semantic rendering (unix_ms → ISO-8601)
  - Enum labels (0 → 'DEBUG', 3 → 'ERROR')
  - Forward compatibility (old readers skip unknown fields)
```

## Key Concepts

### Msgpack Numeric Tags

Define structs with numeric msgpack tags:

```go
type LogEntry struct {
    Timestamp uint64            `msgpack:"1"`
    Level     uint8             `msgpack:"2"`
    Message   string            `msgpack:"3"`
    Tags      map[string]string `msgpack:"4"`
}
```

**Why numeric tags?**
- **Forward compatibility**: Old readers ignore unknown tags
- **Schema evolution**: Add fields without breaking old code
- **Compact wire format**: Numbers are smaller than field names
- **Type registry**: Maps tags to human-readable field names

### Type Registry Bundle

The `bundle.json` file describes the type:

```json
{
  "registry_version": 1,
  "bundle_id": "com.example.logs-v1",
  "types": {
    "com.example.LogEntry": {
      "versions": {
        "1": {
          "fields": {
            "1": { "name": "timestamp", "type": "u64", "semantic": "unix_ms" },
            "2": { "name": "level", "type": "u8", "enum": "com.example.LogLevel" },
            "3": { "name": "message", "type": "string" },
            "4": { "name": "tags", "type": "map", "optional": true }
          }
        }
      }
    }
  },
  "enums": {
    "com.example.LogLevel": {
      "0": "DEBUG",
      "1": "INFO",
      "2": "WARN",
      "3": "ERROR"
    }
  }
}
```

**Bundle components:**
- `registry_version`: Format version (always 1)
- `bundle_id`: Unique identifier for this bundle
- `types`: Type definitions with field descriptors
- `enums`: Enum value mappings

### Field Descriptors

Each field has:
- **name**: JSON projection field name
- **type**: Scalar, array, or map type
- **semantic**: Rendering hint (unix_ms, url, markdown, etc.)
- **enum**: Reference to enum definition
- **optional**: Whether field may be missing (default: false)

### Publishing Bundles

Publish via HTTP API:

```bash
curl -X PUT http://localhost:9010/v1/registry/bundles/my-bundle-id \
  -H "Content-Type: application/json" \
  -d @bundle.json
```

Or programmatically:

```go
req, _ := http.NewRequest("PUT", bundleURL, bytes.NewReader(bundleData))
req.Header.Set("Content-Type", "application/json")
resp, _ := http.DefaultClient.Do(req)
```

**When to publish:**
- Before appending turns with custom types
- On application startup
- After schema changes (new versions)

### Semantic Hints

Improve UI rendering with semantic hints:

| Semantic | Type | Rendering |
|----------|------|-----------|
| `unix_ms` | u64 | ISO-8601 timestamp |
| `unix_sec` | u64 | ISO-8601 timestamp |
| `duration_ms` | u64 | Human duration (e.g., "2h 15m") |
| `url` | string | Clickable link |
| `markdown` | string | Rendered markdown |

Example:

```json
{
  "1": {
    "name": "timestamp",
    "type": "u64",
    "semantic": "unix_ms"
  }
}
```

Projection result:

```json
{
  "timestamp": "2025-01-30T10:00:00.000Z"
}
```

## Schema Evolution

### Adding a Field (Safe)

**Version 1:**
```go
type LogEntry struct {
    Timestamp uint64 `msgpack:"1"`
    Level     uint8  `msgpack:"2"`
    Message   string `msgpack:"3"`
}
```

**Version 2:** (add Tags field)
```go
type LogEntry struct {
    Timestamp uint64            `msgpack:"1"`
    Level     uint8             `msgpack:"2"`
    Message   string            `msgpack:"3"`
    Tags      map[string]string `msgpack:"4"` // New field
}
```

**Registry:**
```json
{
  "versions": {
    "1": { "fields": { "1": {...}, "2": {...}, "3": {...} } },
    "2": {
      "fields": {
        "1": {...},
        "2": {...},
        "3": {...},
        "4": { "name": "tags", "type": "map", "optional": true }
      }
    }
  }
}
```

**Compatibility:**
- Old writers produce v1 (3 fields)
- New writers produce v2 (4 fields)
- Old readers skip tag 4 (unknown field)
- New readers treat missing tag 4 as null

### Changing a Field Type (Unsafe - Use New Tag)

**Wrong:**
```diff
- "2": { "name": "level", "type": "u8" }
+ "2": { "name": "level", "type": "string" }  // BREAKS old readers!
```

**Correct:**
```go
type LogEntry struct {
    Timestamp uint64 `msgpack:"1"`
    // Level  uint8  `msgpack:"2"`  // Deprecated
    Message   string `msgpack:"3"`
    LevelName string `msgpack:"5"`  // New tag
}
```

**Never reuse tags** - assign a new tag number.

## View Typed Projection

After running the example, view the typed JSON in your browser:

```
http://localhost:9010/contexts/1/turns?view=typed
```

Compare with raw msgpack:

```
http://localhost:9010/contexts/1/turns?view=raw
```

**Raw view** (msgpack with numeric keys):
```json
{
  "1": 1706615000000,
  "2": 1,
  "3": "Application started",
  "4": {"version": "1.0.0", "env": "production"}
}
```

**Typed view** (projected with registry):
```json
{
  "timestamp": "2025-01-30T10:00:00.000Z",
  "level": "INFO",
  "message": "Application started",
  "tags": {"version": "1.0.0", "env": "production"}
}
```

## Troubleshooting

### Bundle Publish Fails

**Error**: `Failed to publish bundle: connection refused`

**Solution**: Ensure the HTTP gateway is running on port 9010:
```bash
# Check server logs - HTTP gateway runs on same process
cargo run --release
```

### Invalid Bundle JSON

**Error**: `409 Conflict: invalid descriptor`

**Solution**: Validate your bundle.json:
- Field tags must be positive integers
- Field types must be valid (u8, u16, u32, u64, i8, i16, i32, i64, f32, f64, bool, string, bytes, array, map)
- Map types need `key_type` and `value_type`
- Array types need `items`

### Type Not Found

**Error**: `424 Failed Dependency: type not found`

**Solution**: Publish the bundle before appending turns. The example does this automatically, but if you restart the server, you need to re-publish.

## Next Steps

- **[Custom Renderer](../renderer-custom/)**: Build UI visualization for LogEntry
- **[Agent Integration](../agent-integration/)**: Use canonical conversation types
- **[Type Registry Docs](../../docs/type-registry.md)**: Complete type registry reference
- **[HTTP API Docs](../../docs/http-api.md)**: HTTP endpoint documentation

## Best Practices

1. **Publish bundles on startup**: Don't assume bundles persist across restarts
2. **Use reverse-domain type IDs**: `com.yourcompany.product.TypeName`
3. **Never reuse tags**: Even if you remove a field
4. **Mark new fields optional**: Old data won't have them
5. **Use semantic hints**: Improve UI rendering
6. **Version on descriptor changes**: Helps track schema evolution
7. **Test evolution**: Write v1 data, read with v2 descriptor (and vice versa)

## License

Copyright 2025 StrongDM Inc
SPDX-License-Identifier: Apache-2.0
