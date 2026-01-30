# Filesystem Snapshot Example

This example demonstrates capturing filesystem state and tracking changes across turns.

## What It Does

1. **Creates** a temporary test directory with sample files
2. **Captures** initial filesystem snapshot (merkle tree)
3. **Uploads** snapshot to CXDB blob store
4. **Appends** a turn with filesystem attachment
5. **Modifies** the filesystem (add/modify/delete files)
6. **Captures** second snapshot
7. **Uploads** and attaches second snapshot
8. **Computes diff** and displays changes

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
CXDB Filesystem Snapshot Example
=================================

Setting up test directory...
Created test directory: /tmp/cxdb-fstree-example
Files:
  - README.md
  - src/main.go
  - src/utils.go
  - config/app.yml

Connecting to CXDB at localhost:9009...
Connected successfully!

Creating new context...
Created context ID: 1

[SNAPSHOT 1] Capturing initial state...
Captured 4 files, 3 trees, 0 symlinks
Root hash: a3f5b8c2
Uploading snapshot to CXDB...
Upload complete!
Appended turn 1 with filesystem attachment

[CHANGES] Modifying filesystem...
  + Added src/server.go
  ~ Modified config/app.yml
  - Deleted src/utils.go

[SNAPSHOT 2] Capturing updated state...
Captured 4 files, 3 trees, 0 symlinks
Root hash: b4e6c9d3
Uploading snapshot to CXDB...
Upload complete!
Appended turn 2 with filesystem attachment

[DIFF] Computing changes between snapshots...

Changes:

  Added files:
    + src/server.go

  Modified files:
    ~ config/app.yml

  Removed files:
    - src/utils.go

======================================================================

Success! Created 2 turns with filesystem attachments.
Context ID: 1
Turn 1: 4 files (hash=a3f5b8c2)
Turn 2: 4 files (hash=b4e6c9d3)

Changes: +1, ~1, -1

View in the UI:
  http://localhost:8080/contexts/1

(Filesystem trees are displayed in the turn's attachment section)
```

## Key Concepts

### Filesystem Snapshots

A snapshot captures the complete state of a directory tree:

```go
snapshot, err := fstree.CaptureSnapshot(path, fstree.DefaultCaptureOptions())
```

**What's captured:**
- Regular files (content-addressed by BLAKE3)
- Directories (merkle trees)
- Symlinks (target path)
- Permissions (mode bits)

**What's NOT captured:**
- File timestamps (non-deterministic)
- Owner/group (environment-specific)
- Extended attributes (platform-specific)

### Merkle Trees

Each directory is a merkle tree:

```
Root Directory (hash=abc123)
├─ src/ (hash=def456)
│  ├─ main.go (hash=789012)
│  └─ utils.go (hash=345678)
└─ config/ (hash=901234)
   └─ app.yml (hash=567890)
```

**Properties:**
- Content-addressed: identical trees have identical hashes
- Efficient deduplication: shared subtrees stored once
- Fast comparison: different hash = different content

### Uploading to CXDB

```go
err := fstree.UploadSnapshot(ctx, client, snapshot)
```

**What happens:**
1. All file contents uploaded as blobs (deduplicated)
2. All tree objects uploaded as blobs
3. Root hash returned for attachment

**Deduplication:**
- Identical files uploaded once (hash-based)
- Shared subtrees reused
- Only changed files consume new storage

### Attaching to Turns

```go
turn, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
    ContextID:   contextID,
    TypeID:      "com.example.Metadata",
    TypeVersion: 1,
    Payload:     payload,
    FSRootHash:  &snapshot.RootHash,  // Filesystem attachment
})
```

**Benefits:**
- Turn payload remains small (just metadata)
- Filesystem stored separately (efficient)
- Multiple turns can reference same trees (dedup)

### Computing Diffs

Compare snapshots to find changes:

```go
type Diff struct {
    Added    []string  // Files in snap2 but not snap1
    Modified []string  // Files with different hashes
    Removed  []string  // Files in snap1 but not snap2
}
```

**Algorithm:**
1. Walk both snapshots to build path → hash maps
2. Compare maps:
   - In snap2 but not snap1 = Added
   - In both but different hash = Modified
   - In snap1 but not snap2 = Removed

## Use Cases

### Build Systems

Track build inputs and outputs:

```go
// Before build
inputSnapshot, _ := fstree.CaptureSnapshot("./src", opts)
// ... run build ...
// After build
outputSnapshot, _ := fstree.CaptureSnapshot("./dist", opts)

// Attach both to turn
turn, _ := client.AppendTurn(ctx, &cxdb.AppendRequest{
    Payload:    buildMetadata,
    FSRootHash: &outputSnapshot.RootHash,
})
```

### Deployment Tracking

Record deployed configurations:

```go
snapshot, _ := fstree.CaptureSnapshot("/etc/myapp", opts)
turn, _ := client.AppendTurn(ctx, &cxdb.AppendRequest{
    Payload:    deploymentInfo,
    FSRootHash: &snapshot.RootHash,
})
```

### Code Generation

Audit generated files:

```go
beforeSnapshot, _ := fstree.CaptureSnapshot("./generated", opts)
// ... run code generator ...
afterSnapshot, _ := fstree.CaptureSnapshot("./generated", opts)

// Diff shows what was generated
diff := computeDiff(beforeSnapshot, afterSnapshot)
```

## Advanced Options

### Capture Options

```go
opts := fstree.CaptureOptions{
    // Exclude patterns (glob syntax)
    Exclude: []string{
        "*.log",
        ".git/**",
        "node_modules/**",
        "target/**",
    },

    // Follow symlinks (default: false)
    FollowSymlinks: false,

    // Max file size (bytes, 0 = unlimited)
    MaxFileSize: 100 * 1024 * 1024, // 100MB
}

snapshot, err := fstree.CaptureSnapshot(path, opts)
```

### Snapshot Walking

Iterate over all entries:

```go
err := snapshot.Walk(func(path string, entry fstree.TreeEntry) error {
    fmt.Printf("%s: %s (hash=%x)\n", entry.Kind, path, entry.Hash[:8])
    return nil
})
```

### File Retrieval

Get file content by hash:

```go
reader, err := snapshot.GetFile(fileHash)
if err != nil {
    return err
}
defer reader.Close()

content, _ := io.ReadAll(reader)
fmt.Println(string(content))
```

## Troubleshooting

### Connection Refused

**Error**: `dial tcp 127.0.0.1:9009: connection refused`

**Solution**: Start the CXDB server:
```bash
cd ../..
cargo run --release
```

### Permission Denied

**Error**: `failed to capture snapshot: permission denied`

**Solution**: Ensure you have read access to the directory being captured.

### File Too Large

**Error**: `file exceeds maximum size`

**Solution**: Set `MaxFileSize` in capture options or exclude large files:
```go
opts := fstree.CaptureOptions{
    Exclude: []string{"*.iso", "*.tar.gz"},
}
```

## Next Steps

- **[Basic Go](../basic-go/)**: Learn core CXDB operations
- **[Type Registration](../type-registration/)**: Define custom types
- **[Client SDK](../../clients/go/fstree/)**: Full fstree package docs
- **[Architecture](../../docs/architecture.md)**: How filesystem trees are stored

## Best Practices

1. **Exclude build artifacts**: Add `node_modules/`, `target/`, `.git/` to exclude list
2. **Limit file sizes**: Set `MaxFileSize` to avoid uploading huge binaries
3. **Capture at milestones**: Before/after significant operations
4. **Use descriptive metadata**: Include operation type, timestamp, user
5. **Compute diffs before upload**: Skip upload if no changes detected
6. **Handle symlinks carefully**: Decide if you want to follow or preserve them

## Performance

**Capture time** (typical laptop):
- 100 files: ~10ms
- 1,000 files: ~100ms
- 10,000 files: ~1s

**Upload time** (localhost):
- 10MB (new): ~50ms
- 10MB (duplicated): ~5ms (dedup)
- 100MB (new): ~500ms

**Storage efficiency**:
- Identical files deduplicated globally
- Shared subtrees stored once
- Only deltas consume new space

## License

Copyright 2025 StrongDM Inc
SPDX-License-Identifier: Apache-2.0
