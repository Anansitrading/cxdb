// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

// Package main demonstrates type registry usage for custom data types.
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"

	"github.com/strongdm/cxdb"
	"github.com/vmihailenco/msgpack/v5"
)

const (
	// TypeID for our custom LogEntry type
	TypeIDLogEntry = "com.example.LogEntry"

	// Current version
	TypeVersionLogEntry uint32 = 1

	// Bundle ID matching bundle.json
	BundleID = "com.example.logs-v1"
)

func main() {
	// Step 1: Read the type registry bundle
	fmt.Println("Reading type registry bundle from bundle.json...")
	bundleData, err := os.ReadFile("bundle.json")
	if err != nil {
		log.Fatalf("Failed to read bundle.json: %v", err)
	}

	// Validate JSON
	var bundle map[string]interface{}
	if err := json.Unmarshal(bundleData, &bundle); err != nil {
		log.Fatalf("Invalid bundle JSON: %v", err)
	}
	fmt.Println("Bundle loaded successfully")

	// Step 2: Publish bundle to server
	fmt.Println("\nPublishing type registry bundle to server...")
	httpAddr := "http://localhost:9010"
	bundleURL := fmt.Sprintf("%s/v1/registry/bundles/%s", httpAddr, BundleID)

	req, err := http.NewRequest("PUT", bundleURL, bytes.NewReader(bundleData))
	if err != nil {
		log.Fatalf("Failed to create request: %v", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		log.Fatalf("Failed to publish bundle: %v (is the server running?)", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK && resp.StatusCode != http.StatusCreated {
		body, _ := io.ReadAll(resp.Body)
		log.Fatalf("Failed to publish bundle: HTTP %d: %s", resp.StatusCode, body)
	}

	fmt.Printf("Bundle published successfully (HTTP %d)\n", resp.StatusCode)

	// Step 3: Connect to CXDB binary protocol
	fmt.Println("\nConnecting to CXDB at localhost:9009...")
	client, err := cxdb.Dial("localhost:9009")
	if err != nil {
		log.Fatalf("Failed to connect: %v", err)
	}
	defer client.Close()
	fmt.Println("Connected successfully!")

	ctx := context.Background()

	// Step 4: Create a context
	fmt.Println("\nCreating new context...")
	ctxResp, err := client.CreateContext(ctx, 0)
	if err != nil {
		log.Fatalf("Failed to create context: %v", err)
	}
	fmt.Printf("Created context ID: %d\n", ctxResp.ContextID)
	contextID := ctxResp.ContextID

	// Step 5: Append log entries with different levels
	logs := []*LogEntry{
		NewLogEntry(LevelInfo, "Application started", map[string]string{
			"version": "1.0.0",
			"env":     "production",
		}),
		NewLogEntry(LevelWarn, "High memory usage detected", map[string]string{
			"usage_mb": "2048",
			"limit_mb": "4096",
		}),
		NewLogEntry(LevelError, "Failed to connect to database", map[string]string{
			"host":  "db.example.com",
			"error": "connection timeout",
		}),
		NewLogEntry(LevelDebug, "Cache hit for user profile", map[string]string{
			"user_id": "12345",
			"key":     "profile:12345",
		}),
	}

	fmt.Println("\nAppending log entries...")
	for i, entry := range logs {
		payload, err := msgpack.Marshal(entry)
		if err != nil {
			log.Fatalf("Failed to marshal log entry: %v", err)
		}

		turn, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
			ContextID:   contextID,
			TypeID:      TypeIDLogEntry,
			TypeVersion: TypeVersionLogEntry,
			Payload:     payload,
		})
		if err != nil {
			log.Fatalf("Failed to append turn: %v", err)
		}

		fmt.Printf("  [%d] %s: %s (turn_id=%d)\n",
			i+1, LevelName(entry.Level), entry.Message, turn.TurnID)
	}

	// Step 6: Retrieve and display logs
	fmt.Println("\nRetrieving log entries...")
	turns, err := client.GetLast(ctx, contextID, cxdb.GetLastOptions{Limit: 10, IncludePayload: true})
	if err != nil {
		log.Fatalf("Failed to retrieve turns: %v", err)
	}

	fmt.Printf("\nRetrieved %d log entries:\n", len(turns))
	fmt.Println("=========================================================================")

	for _, turn := range turns {
		var entry LogEntry
		if err := msgpack.Unmarshal(turn.Payload, &entry); err != nil {
			fmt.Printf("Error decoding turn %d: %v\n", turn.TurnID, err)
			continue
		}

		fmt.Printf("\n[Turn %d] %s - %s\n",
			turn.TurnID,
			LevelName(entry.Level),
			entry.Message)
		fmt.Printf("  Timestamp: %d (unix_ms)\n", entry.Timestamp)
		if len(entry.Tags) > 0 {
			fmt.Printf("  Tags:\n")
			for k, v := range entry.Tags {
				fmt.Printf("    %s: %s\n", k, v)
			}
		}
	}

	fmt.Println("\n=========================================================================")
	fmt.Println("\nSuccess! View the typed JSON projection in the UI:")
	fmt.Printf("  %s/contexts/%d/turns?view=typed\n", httpAddr, contextID)
	fmt.Println("\nThe type registry enables:")
	fmt.Println("  - Numeric tags → field names (e.g., 1 → 'timestamp')")
	fmt.Println("  - Semantic rendering (unix_ms → ISO-8601)")
	fmt.Println("  - Enum labels (0 → 'DEBUG', 3 → 'ERROR')")
	fmt.Println("  - Forward compatibility (old readers skip unknown fields)")
}
