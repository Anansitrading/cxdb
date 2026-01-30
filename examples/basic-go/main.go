// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

// Package main demonstrates basic CXDB operations using the Go client SDK.
package main

import (
	"context"
	"fmt"
	"log"
	"strings"

	"github.com/strongdm/cxdb"
	"github.com/vmihailenco/msgpack/v5"
)

// Message represents a simple conversation message with numeric msgpack tags.
// Using numeric tags (not string keys) enables forward-compatible schema evolution.
type Message struct {
	Role string `msgpack:"1"`
	Text string `msgpack:"2"`
}

// ToolCall represents a function invocation request.
type ToolCall struct {
	Name      string                 `msgpack:"1"`
	Arguments map[string]interface{} `msgpack:"2"`
}

func main() {
	// Step 1: Connect to CXDB
	// For local development, use plain TCP. For production, use cxdb.DialTLS().
	fmt.Println("Connecting to CXDB at localhost:9009...")
	client, err := cxdb.Dial("localhost:9009")
	if err != nil {
		log.Fatalf("Failed to connect: %v", err)
	}
	defer client.Close()
	fmt.Println("Connected successfully!")

	ctx := context.Background()

	// Step 2: Create a context
	// A context is a branch head that tracks the latest turn in a conversation.
	fmt.Println("\nCreating new context...")
	ctxResp, err := client.CreateContext(ctx, 0)
	if err != nil {
		log.Fatalf("Failed to create context: %v", err)
	}
	fmt.Printf("Created context ID: %d (head_turn_id=%d, depth=%d)\n",
		ctxResp.ContextID, ctxResp.HeadTurnID, ctxResp.HeadDepth)

	contextID := ctxResp.ContextID

	// Step 3: Append a user turn
	fmt.Println("\nAppending user turn...")
	userMsg := Message{
		Role: "user",
		Text: "What is the weather in San Francisco?",
	}
	userPayload, err := msgpack.Marshal(userMsg)
	if err != nil {
		log.Fatalf("Failed to marshal user message: %v", err)
	}

	userTurn, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
		ContextID:   contextID,
		TypeID:      "com.example.Message",
		TypeVersion: 1,
		Payload:     userPayload,
	})
	if err != nil {
		log.Fatalf("Failed to append user turn: %v", err)
	}
	fmt.Printf("Appended user turn: turn_id=%d, depth=%d, hash=%x\n",
		userTurn.TurnID, userTurn.Depth, userTurn.PayloadHash[:8])

	// Step 4: Append an assistant turn
	fmt.Println("\nAppending assistant turn...")
	assistantMsg := Message{
		Role: "assistant",
		Text: "Let me check the weather for you.",
	}
	assistantPayload, err := msgpack.Marshal(assistantMsg)
	if err != nil {
		log.Fatalf("Failed to marshal assistant message: %v", err)
	}

	assistantTurn, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
		ContextID:   contextID,
		TypeID:      "com.example.Message",
		TypeVersion: 1,
		Payload:     assistantPayload,
	})
	if err != nil {
		log.Fatalf("Failed to append assistant turn: %v", err)
	}
	fmt.Printf("Appended assistant turn: turn_id=%d, depth=%d\n",
		assistantTurn.TurnID, assistantTurn.Depth)

	// Step 5: Append a tool call turn
	fmt.Println("\nAppending tool call turn...")
	toolCallMsg := ToolCall{
		Name: "get_weather",
		Arguments: map[string]interface{}{
			"location": "San Francisco, CA",
			"units":    "fahrenheit",
		},
	}
	toolCallPayload, err := msgpack.Marshal(toolCallMsg)
	if err != nil {
		log.Fatalf("Failed to marshal tool call: %v", err)
	}

	toolTurn, err := client.AppendTurn(ctx, &cxdb.AppendRequest{
		ContextID:   contextID,
		TypeID:      "com.example.ToolCall",
		TypeVersion: 1,
		Payload:     toolCallPayload,
	})
	if err != nil {
		log.Fatalf("Failed to append tool call turn: %v", err)
	}
	fmt.Printf("Appended tool call turn: turn_id=%d, depth=%d\n",
		toolTurn.TurnID, toolTurn.Depth)

	// Step 6: Retrieve the last 10 turns
	fmt.Println("\nRetrieving conversation history...")
	turns, err := client.GetLast(ctx, contextID, cxdb.GetLastOptions{Limit: 10, IncludePayload: true})
	if err != nil {
		log.Fatalf("Failed to retrieve turns: %v", err)
	}

	fmt.Printf("\nConversation history (%d turns):\n", len(turns))
	fmt.Println(strings.Repeat("=", 70))

	for _, turn := range turns {
		fmt.Printf("\nTurn %d (depth=%d, hash=%x...)\n",
			turn.TurnID, turn.Depth, turn.PayloadHash[:8])
		fmt.Printf("  Type: %s v%d\n", turn.TypeID, turn.TypeVersion)

		// Decode based on type
		switch turn.TypeID {
		case "com.example.Message":
			var msg Message
			if err := msgpack.Unmarshal(turn.Payload, &msg); err != nil {
				fmt.Printf("  Error decoding: %v\n", err)
				continue
			}
			fmt.Printf("  Role: %s\n", msg.Role)
			fmt.Printf("  Text: %s\n", msg.Text)

		case "com.example.ToolCall":
			var tc ToolCall
			if err := msgpack.Unmarshal(turn.Payload, &tc); err != nil {
				fmt.Printf("  Error decoding: %v\n", err)
				continue
			}
			fmt.Printf("  Tool: %s\n", tc.Name)
			fmt.Printf("  Arguments: %+v\n", tc.Arguments)

		default:
			fmt.Printf("  Unknown type (raw bytes: %d)\n", len(turn.Payload))
		}
	}

	fmt.Println("\n" + strings.Repeat("=", 70))
	fmt.Println("\nSuccess! View this conversation in the UI:")
	fmt.Printf("  http://localhost:8080/contexts/%d\n", contextID)
	fmt.Println("\n(Start the gateway with: cd ../../gateway && go run ./cmd/server)")
}
