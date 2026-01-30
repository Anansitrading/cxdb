#!/usr/bin/env python3
# Copyright 2025 StrongDM Inc
# SPDX-License-Identifier: Apache-2.0

"""
AI Agent Integration Example

Demonstrates using CXDB to log AI agent conversations via the HTTP API.
Uses canonical ConversationItem types for rich UI visualization.
"""

import requests
import json
import time
from typing import Dict, List, Any, Optional
from datetime import datetime


# CXDB HTTP API endpoint
CXDB_HTTP = "http://localhost:9010"

# Canonical type constants
TYPE_ID = "cxdb.ConversationItem"
TYPE_VERSION = 3


def create_context() -> int:
    """Create a new CXDB context."""
    resp = requests.post(f"{CXDB_HTTP}/v1/contexts/create")
    resp.raise_for_status()
    data = resp.json()
    return int(data["context_id"])


def append_turn(context_id: int, item: Dict[str, Any]) -> Dict[str, Any]:
    """Append a ConversationItem turn to the context."""
    resp = requests.post(
        f"{CXDB_HTTP}/v1/contexts/{context_id}/append",
        json={
            "type_id": TYPE_ID,
            "type_version": TYPE_VERSION,
            "data": item,
        },
    )
    resp.raise_for_status()
    return resp.json()


def get_turns(context_id: int, limit: int = 100) -> List[Dict[str, Any]]:
    """Retrieve turns from the context."""
    resp = requests.get(
        f"{CXDB_HTTP}/v1/contexts/{context_id}/turns",
        params={"limit": limit},
    )
    resp.raise_for_status()
    data = resp.json()
    return data.get("turns", [])


def timestamp_ms() -> int:
    """Get current timestamp in milliseconds."""
    return int(time.time() * 1000)


def user_input(text: str, attachments: Optional[List[Dict]] = None) -> Dict:
    """Create a user input ConversationItem."""
    return {
        "item_type": "user_input",
        "timestamp": timestamp_ms(),
        "text": text,
        "attachments": attachments or [],
    }


def assistant_turn(
    text: str,
    tool_calls: Optional[List[Dict]] = None,
    status: str = "complete",
) -> Dict:
    """Create an assistant turn ConversationItem with nested tool calls."""
    return {
        "item_type": "assistant_turn",
        "timestamp": timestamp_ms(),
        "text": text,
        "tool_calls": tool_calls or [],
        "status": status,
    }


def tool_call(name: str, arguments: Dict, call_id: str) -> Dict:
    """Create a tool call for embedding in assistant_turn."""
    return {
        "call_id": call_id,
        "name": name,
        "arguments": arguments,
        "status": "complete",
        "result": None,  # Will be filled later
    }


def system_message(text: str, severity: str = "info") -> Dict:
    """Create a system message ConversationItem."""
    return {
        "item_type": "system",
        "timestamp": timestamp_ms(),
        "text": text,
        "severity": severity,
    }


def simulate_agent_conversation(context_id: int):
    """Simulate a multi-turn AI agent conversation."""
    print(f"\nSimulating agent conversation in context {context_id}...\n")

    # Turn 1: User asks a question
    print("[Turn 1] User input")
    turn1 = append_turn(
        context_id,
        user_input("What's the weather like in San Francisco?"),
    )
    print(f"  Appended turn {turn1['turn_id']}")

    # Turn 2: Assistant decides to call a tool
    print("[Turn 2] Assistant + tool call")
    turn2 = append_turn(
        context_id,
        assistant_turn(
            text="Let me check the current weather for you.",
            tool_calls=[
                {
                    "call_id": "call_001",
                    "name": "get_weather",
                    "arguments": {
                        "location": "San Francisco, CA",
                        "units": "fahrenheit",
                    },
                    "status": "complete",
                    "result": {
                        "temperature": 62,
                        "conditions": "partly cloudy",
                        "humidity": 65,
                        "wind_speed": 12,
                    },
                }
            ],
        ),
    )
    print(f"  Appended turn {turn2['turn_id']}")

    # Turn 3: Assistant provides final answer
    print("[Turn 3] Assistant response")
    turn3 = append_turn(
        context_id,
        assistant_turn(
            text="The weather in San Francisco is currently 62°F and partly cloudy. "
            "Humidity is at 65% with winds at 12 mph. It's a pleasant day!",
        ),
    )
    print(f"  Appended turn {turn3['turn_id']}")

    # Turn 4: User asks a follow-up
    print("[Turn 4] User follow-up")
    turn4 = append_turn(
        context_id,
        user_input("Should I bring a jacket?"),
    )
    print(f"  Appended turn {turn4['turn_id']}")

    # Turn 5: Assistant responds
    print("[Turn 5] Assistant response")
    turn5 = append_turn(
        context_id,
        assistant_turn(
            text="Yes, I'd recommend bringing a light jacket. While 62°F is mild, "
            "San Francisco can feel cooler with the wind and fog, especially near the coast.",
        ),
    )
    print(f"  Appended turn {turn5['turn_id']}")

    # Turn 6: System message (for demonstration)
    print("[Turn 6] System info")
    turn6 = append_turn(
        context_id,
        system_message(
            "Conversation token usage: 450 tokens (350 input, 100 output)",
            severity="info",
        ),
    )
    print(f"  Appended turn {turn6['turn_id']}")


def display_conversation(context_id: int):
    """Retrieve and display the conversation."""
    print(f"\n{'=' * 70}")
    print(f"Conversation (Context {context_id})")
    print("=" * 70)

    turns = get_turns(context_id)

    for turn in turns:
        data = turn.get("data", {})
        item_type = data.get("item_type", "unknown")
        timestamp = data.get("timestamp", 0)
        dt = datetime.fromtimestamp(timestamp / 1000).strftime("%H:%M:%S")

        print(f"\n[Turn {turn['turn_id']}] {item_type.upper()} @ {dt}")

        if item_type == "user_input":
            print(f"  User: {data.get('text', '')}")
            attachments = data.get("attachments", [])
            if attachments:
                print(f"  Attachments: {len(attachments)}")

        elif item_type == "assistant_turn":
            print(f"  Assistant: {data.get('text', '')}")
            tool_calls = data.get("tool_calls", [])
            if tool_calls:
                print(f"\n  Tool calls:")
                for tc in tool_calls:
                    print(f"    - {tc['name']}({json.dumps(tc['arguments'])})")
                    if tc.get("result"):
                        print(f"      → {json.dumps(tc['result'], indent=8)}")

        elif item_type == "system":
            severity = data.get("severity", "info")
            print(f"  [{severity.upper()}] {data.get('text', '')}")

        else:
            print(f"  (Unknown item type: {item_type})")

    print(f"\n{'=' * 70}\n")


def main():
    """Main entry point."""
    print("AI Agent Integration Example")
    print("=" * 70)

    # Create a context
    print("\nCreating context...")
    context_id = create_context()
    print(f"Created context {context_id}")

    # Simulate conversation
    simulate_agent_conversation(context_id)

    # Display the conversation
    display_conversation(context_id)

    # Summary
    print("\nSuccess! View the conversation in the UI:")
    print(f"  {CXDB_HTTP}/contexts/{context_id}")
    print("\nThe UI provides:")
    print("  - Rich conversation visualization")
    print("  - Tool call inspection")
    print("  - Timeline view")
    print("  - Turn-by-turn navigation")
    print("\n(Start the gateway with: cd ../../gateway && go run ./cmd/server)")


if __name__ == "__main__":
    try:
        main()
    except requests.exceptions.ConnectionError:
        print("\nError: Could not connect to CXDB HTTP API")
        print("Ensure the server is running on http://localhost:9010")
        print("\nStart the server with:")
        print("  cd ../.. && cargo run --release")
        exit(1)
    except Exception as e:
        print(f"\nError: {e}")
        exit(1)
