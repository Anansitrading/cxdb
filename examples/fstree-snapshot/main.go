// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

// Package main demonstrates capturing filesystem snapshots and tracking changes across turns.
package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"strings"

	cxdb "github.com/strongdm/ai-cxdb/clients/go"
	"github.com/strongdm/ai-cxdb/clients/go/fstree"
	"github.com/vmihailenco/msgpack/v5"
)

// Metadata represents turn metadata with filesystem attachment.
type Metadata struct {
	Description string `msgpack:"1"`
	Operation   string `msgpack:"2"`
}

func main() {
	fmt.Println("CXDB Filesystem Snapshot Example")
	fmt.Println("=================================\n")

	// Step 1: Create a temporary directory for testing
	fmt.Println("Setting up test directory...")
	testDir := filepath.Join(os.TempDir(), "cxdb-fstree-example")
	if err := os.RemoveAll(testDir); err != nil {
		log.Fatalf("Failed to clean test directory: %v", err)
	}
	if err := os.MkdirAll(testDir, 0755); err != nil {
		log.Fatalf("Failed to create test directory: %v", err)
	}
	defer os.RemoveAll(testDir) // Clean up after

	// Create some initial files
	files := map[string]string{
		"README.md":      "# My Project\n\nThis is a test project.",
		"src/main.go":    "package main\n\nfunc main() {}\n",
		"src/utils.go":   "package main\n\nfunc helper() string { return \"hi\" }\n",
		"config/app.yml": "version: 1.0.0\nport: 8080\n",
	}

	for path, content := range files {
		fullPath := filepath.Join(testDir, path)
		if err := os.MkdirAll(filepath.Dir(fullPath), 0755); err != nil {
			log.Fatalf("Failed to create directory: %v", err)
		}
		if err := os.WriteFile(fullPath, []byte(content), 0644); err != nil {
			log.Fatalf("Failed to write file: %v", err)
		}
	}

	fmt.Printf("Created test directory: %s\n", testDir)
	fmt.Println("Files:")
	for path := range files {
		fmt.Printf("  - %s\n", path)
	}

	// Step 2: Connect to CXDB
	fmt.Println("\nConnecting to CXDB at localhost:9009...")
	client, err := cxdb.Dial("localhost:9009")
	if err != nil {
		log.Fatalf("Failed to connect: %v", err)
	}
	defer client.Close()
	fmt.Println("Connected successfully!")

	ctx := context.Background()

	// Step 3: Create a context
	fmt.Println("\nCreating new context...")
	ctxResp, err := client.CreateContext(ctx, 0)
	if err != nil {
		log.Fatalf("Failed to create context: %v", err)
	}
	fmt.Printf("Created context ID: %d\n", ctxResp.ContextID)
	contextID := ctxResp.ContextID

	// Step 4: Capture initial snapshot
	fmt.Println("\n[SNAPSHOT 1] Capturing initial state...")
	snapshot1, err := fstree.Capture(testDir)
	if err != nil {
		log.Fatalf("Failed to capture snapshot: %v", err)
	}

	fmt.Printf("Captured %d files, %d trees, %d symlinks\n",
		len(snapshot1.Files), len(snapshot1.Trees), len(snapshot1.Symlinks))
	fmt.Printf("Root hash: %x\n", snapshot1.RootHash[:8])

	// Upload to CXDB
	fmt.Println("Uploading snapshot to CXDB...")
	_, err = snapshot1.Upload(ctx, client)
	if err != nil {
		log.Fatalf("Failed to upload snapshot: %v", err)
	}
	fmt.Println("Upload complete!")

	// Append a turn with the snapshot
	metadata1 := Metadata{
		Description: "Initial project state",
		Operation:   "init",
	}
	payload1, _ := msgpack.Marshal(metadata1)

	turn1, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
		ContextID:   contextID,
		TypeID:      "com.example.Metadata",
		TypeVersion: 1,
		Payload:     payload1,
	})
	if err != nil {
		log.Fatalf("Failed to append turn: %v", err)
	}
	fmt.Printf("Appended turn %d\n", turn1.TurnID)

	// Attach filesystem snapshot to the turn
	fmt.Println("Attaching filesystem snapshot to turn...")
	_, err = client.AttachFs(ctx, &cxdb.AttachFsRequest{
		TurnID:     turn1.TurnID,
		FsRootHash: snapshot1.RootHash,
	})
	if err != nil {
		log.Fatalf("Failed to attach filesystem: %v", err)
	}
	fmt.Println("Filesystem attached!")

	// Step 5: Make some changes
	fmt.Println("\n[CHANGES] Modifying filesystem...")

	// Add a new file
	newFile := filepath.Join(testDir, "src/server.go")
	newContent := "package main\n\nimport \"net/http\"\n\nfunc startServer() {}\n"
	if err := os.WriteFile(newFile, []byte(newContent), 0644); err != nil {
		log.Fatalf("Failed to write new file: %v", err)
	}
	fmt.Println("  + Added src/server.go")

	// Modify an existing file
	modFile := filepath.Join(testDir, "config/app.yml")
	modContent := "version: 1.1.0\nport: 8080\nlog_level: debug\n"
	if err := os.WriteFile(modFile, []byte(modContent), 0644); err != nil {
		log.Fatalf("Failed to modify file: %v", err)
	}
	fmt.Println("  ~ Modified config/app.yml")

	// Delete a file
	delFile := filepath.Join(testDir, "src/utils.go")
	if err := os.Remove(delFile); err != nil {
		log.Fatalf("Failed to delete file: %v", err)
	}
	fmt.Println("  - Deleted src/utils.go")

	// Step 6: Capture second snapshot
	fmt.Println("\n[SNAPSHOT 2] Capturing updated state...")
	snapshot2, err := fstree.Capture(testDir)
	if err != nil {
		log.Fatalf("Failed to capture snapshot: %v", err)
	}

	fmt.Printf("Captured %d files, %d trees, %d symlinks\n",
		len(snapshot2.Files), len(snapshot2.Trees), len(snapshot2.Symlinks))
	fmt.Printf("Root hash: %x\n", snapshot2.RootHash[:8])

	// Upload second snapshot
	fmt.Println("Uploading snapshot to CXDB...")
	_, err = snapshot2.Upload(ctx, client)
	if err != nil {
		log.Fatalf("Failed to upload snapshot: %v", err)
	}
	fmt.Println("Upload complete!")

	// Append second turn
	metadata2 := Metadata{
		Description: "Added server, updated config, removed unused utils",
		Operation:   "refactor",
	}
	payload2, _ := msgpack.Marshal(metadata2)

	turn2, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
		ContextID:   contextID,
		TypeID:      "com.example.Metadata",
		TypeVersion: 1,
		Payload:     payload2,
	})
	if err != nil {
		log.Fatalf("Failed to append turn: %v", err)
	}
	fmt.Printf("Appended turn %d\n", turn2.TurnID)

	// Attach filesystem snapshot to the turn
	fmt.Println("Attaching filesystem snapshot to turn...")
	_, err = client.AttachFs(ctx, &cxdb.AttachFsRequest{
		TurnID:     turn2.TurnID,
		FsRootHash: snapshot2.RootHash,
	})
	if err != nil {
		log.Fatalf("Failed to attach filesystem: %v", err)
	}
	fmt.Println("Filesystem attached!")

	// Step 7: Compute and display diff
	fmt.Println("\n[DIFF] Computing changes between snapshots...")
	diff, err := snapshot2.Diff(snapshot1)
	if err != nil {
		log.Fatalf("Failed to compute diff: %v", err)
	}

	fmt.Println("\nChanges:")
	if len(diff.Added) > 0 {
		fmt.Println("\n  Added files:")
		for _, path := range diff.Added {
			fmt.Printf("    + %s\n", path)
		}
	}

	if len(diff.Modified) > 0 {
		fmt.Println("\n  Modified files:")
		for _, path := range diff.Modified {
			fmt.Printf("    ~ %s\n", path)
		}
	}

	if len(diff.Removed) > 0 {
		fmt.Println("\n  Removed files:")
		for _, path := range diff.Removed {
			fmt.Printf("    - %s\n", path)
		}
	}

	if len(diff.Added) == 0 && len(diff.Modified) == 0 && len(diff.Removed) == 0 {
		fmt.Println("  No changes detected")
	}

	// Summary
	fmt.Println("\n" + strings.Repeat("=", 70))
	fmt.Printf("\nSuccess! Created %d turns with filesystem attachments.\n", 2)
	fmt.Printf("Context ID: %d\n", contextID)
	fmt.Printf("Turn 1: %d files (hash=%x)\n", len(snapshot1.Files), snapshot1.RootHash[:8])
	fmt.Printf("Turn 2: %d files (hash=%x)\n", len(snapshot2.Files), snapshot2.RootHash[:8])
	fmt.Printf("\nChanges: +%d, ~%d, -%d\n",
		len(diff.Added), len(diff.Modified), len(diff.Removed))

	fmt.Println("\nView in the UI:")
	fmt.Printf("  http://localhost:8080/contexts/%d\n", contextID)
	fmt.Println("\n(Filesystem trees are displayed in the turn's attachment section)")
}
