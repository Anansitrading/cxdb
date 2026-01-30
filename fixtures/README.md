# CXDB Test Fixtures

Generated test data for CXDB development and testing.

## Directories

- `protocol/` - Binary protocol message fixtures (Go-generated)
- `fstree/` - Filesystem snapshot fixtures (Go-generated + sample directory)
- `types/` - Canonical conversation type fixtures (Go-generated)
- `registry/` - Type registry bundle examples (hand-crafted)

## Regenerating Fixtures

To regenerate all fixtures:

```bash
./scripts/generate-fixtures.sh
```

To regenerate specific fixture sets, use the Go fixture generator programs directly:

```bash
# Protocol messages
cd clients/go && go run ./cmd/cxdb-fixtures -out ../../fixtures/protocol

# Filesystem snapshots
cd clients/go && go run ./cmd/cxdb-fstree-fixtures -out ../../fixtures/fstree

# Msgpack types
cd clients/go && go run ./cmd/cxdb-msgpack-fixtures -out ../../fixtures/types
```

## Usage in Tests

Rust tests load fixtures from relative paths:
```rust
let fixture = include_bytes!("../../fixtures/protocol/hello.bin");
```

Go tests load fixtures from relative paths:
```go
data, err := os.ReadFile("../../fixtures/protocol/hello.bin")
```
