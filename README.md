# cxdb - Content-Addressed Context Store with Turn DAG

Rust server providing immutable conversation storage with O(1) forking. Every conversation is a chain of immutable turns in a directed acyclic graph (Turn DAG). Payloads are deduplicated via BLAKE3 content-addressed storage and compressed with Zstd. Writes go over a binary protocol on `:9009`, reads over an HTTP API on `:9010`. A type registry enables structured, typed JSON projections of the underlying msgpack data.

Forked from [StrongDM/cxdb](https://github.com/strongdm/cxdb) (Apache 2.0). This fork is maintained at [Anansitrading/cxdb](https://github.com/Anansitrading/cxdb).

## Our Deployment (Oracle-Cortex Integration)

This fork extends cxdb with Oracle-Cortex specific tooling. The Cortex-specific code lives in the [Oracle-Cortex](https://github.com/Anansitrading/Oracle-Cortex) repo since it depends on the broader cognitive architecture. This repo contains only the upstream Rust server code and deployment config.

- **Python Client**: `Oracle-Cortex/scripts/cortex/cxdb_client.py` - Full SDK speaking the binary protocol
- **Integration Module**: `Oracle-Cortex/scripts/cortex/cxdb_integration.py` - SessionRecorder, BranchExplorer, SessionBrowser
- **Zulip Bot**: `Oracle-Cortex/scripts/bots/cxdb_bot.py` - Chat interface on `#cxdb` channel
- **Documentation**: `Oracle-Cortex/docs/cxdb-conversation-branching.md`

## Architecture

```
Oracle/Smith/Trinity sessions
        |
        v
  Python Client (Oracle-Cortex/scripts/cortex/cxdb_client.py)
        |
        v  (binary :9009 writes, HTTP :9010 reads)
  cxdb-server (this repo, Rust)
        |
        v
  ~/.cxdb/data/
    turns/    (Turn DAG, 104 bytes per turn)
    blobs/    (Content-addressed payloads, BLAKE3 + Zstd)
    registry/ (Type descriptors for JSON projection)
```

## Building from Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
cargo build --release

# Install
sudo cp target/release/cxdb-server /usr/local/bin/
```

## Service Management

```bash
# Service file: /etc/systemd/system/cxdb.service (source: cxdb.service in this repo)
sudo systemctl enable cxdb
sudo systemctl start cxdb
sudo systemctl status cxdb

# Logs
sudo journalctl -u cxdb -f
```

## Configuration

CXDB is configured via environment variables. See `cxdb.service` in this repo for the deployed values.

| Variable | Default | Purpose |
|----------|---------|---------|
| `CXDB_DATA_DIR` | `~/.cxdb/data` | Storage directory |
| `CXDB_BIND` | `127.0.0.1:9009` | Binary protocol address |
| `CXDB_HTTP_BIND` | `127.0.0.1:9010` | HTTP API address |
| `CXDB_LOG_LEVEL` | `info` | Logging verbosity |
| `CXDB_MAX_BLOB_SIZE` | `10485760` | Max payload size (10MB) |
| `CXDB_COMPRESSION_LEVEL` | `3` | Zstd compression level (1-22) |

## HTTP API (Read-Only)

```bash
# Health check
curl http://localhost:9010/healthz

# List contexts
curl http://localhost:9010/v1/contexts

# Get turns with typed projection
curl "http://localhost:9010/v1/contexts/1/turns?limit=20&view=typed"

# View registry
curl http://localhost:9010/v1/registry/bundles/2026-02-08T01:00:00Z%23oracle-cortex-v1
```

## Registered Types (Oracle-Cortex)

- `com.oracle.conversation.Turn` (v1) - role, content, timestamp, metadata
- `com.oracle.agent.ToolCall` (v1) - tool_name, input, output, timestamp, duration_ms, status
- `com.oracle.agent.SessionMeta` (v1) - session_id, agent, started_at, trigger, zulip_stream, zulip_topic

## Where cxdb Fits in Oracle-Cortex Memory Stack

```
HOT (real-time)    Graphiti (FalkorDB)     <-- entities, relations, semantic
                          |
BRANCHING          cxdb (Turn DAG)         <-- conversation history, forks, RL
                          |
FORENSIC           DuckDB                  <-- SQL queries over sessions
                          |
COLD               Google File Search      <-- RAG over all sessions
                          |
HUMAN              NotebookLM              <-- audio briefings, transparency
```

## Related Repositories

- [Oracle-Cortex](https://github.com/Anansitrading/Oracle-Cortex) - Parent cognitive architecture (Python client, integration, bot, docs)
- [Kijko-Swarm](https://github.com/Anansitrading/Kijko-Swarm) - Zulip communications hub (`#cxdb` channel)

## Upstream Documentation

The original StrongDM docs are preserved in `docs/`:

- [Getting Started](docs/getting-started.md)
- [Architecture](docs/architecture.md)
- [Binary Protocol](docs/protocol.md)
- [HTTP API](docs/http-api.md)
- [Type Registry](docs/type-registry.md)
- [Renderers](docs/renderers.md)
- [Deployment](docs/deployment.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Development](docs/development.md)

## License

Forked from [StrongDM/cxdb](https://github.com/strongdm/cxdb). Licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.
