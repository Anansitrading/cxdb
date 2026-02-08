#!/usr/bin/env python3
"""
cxdb Zulip Bot - Conversation Branching Interface

Listens on #cxdb channel. Commands:
  sessions                       → list recent contexts
  show CTX-<id>                  → show turns from a context
  fork CTX-<id>:<turn> "reason"  → fork at a turn, create new topic
  compare CTX-<id> CTX-<id> ...  → side-by-side branch comparison
  score CTX-<id> <reward> "why"  → attach reward signal to a branch
  search <query>                 → search across all contexts
  help                           → show commands

Each context maps to a Zulip topic: [CTX-N] description
Fork = new topic + new cxdb branch.
"""

import logging
import os
import re
import signal
import sys
import time
from pathlib import Path

import zulip

# Add Oracle-Cortex scripts to path for cxdb client
sys.path.insert(0, "/home/devuser/Oracle-Cortex/scripts")

from cortex.cxdb_client import CxdbClient, CxdbError
from cortex.cxdb_integration import SessionRecorder, BranchExplorer, SessionBrowser

# Configuration
ZULIP_RC = Path.home() / "Zulip/bots/cxdb-zuliprc"
CHANNEL = "cxdb"
BOT_EMAIL_PREFIX = "cxdb-bot"
LOG_DIR = Path.home() / ".cxdb/logs"
PID_FILE = Path.home() / ".cxdb/cxdb-bot.pid"

# Logging
LOG_DIR.mkdir(parents=True, exist_ok=True)
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers=[
        logging.FileHandler(LOG_DIR / "bot.log"),
        logging.StreamHandler(),
    ],
)
log = logging.getLogger("cxdb-bot")

# Regex patterns
CTX_PATTERN = re.compile(r'CTX-(\d+)')
FORK_PATTERN = re.compile(r'CTX-(\d+):(\d+)')


class CxdbBot:
    """Zulip bot for cxdb conversation branching."""

    def __init__(self):
        self.zulip = zulip.Client(config_file=str(ZULIP_RC))
        self.cxdb = CxdbClient(client_tag="cxdb-zulip-bot")
        self.explorer = BranchExplorer(client=self.cxdb)
        self.browser = SessionBrowser(client=self.cxdb)
        self.running = True

    def start(self):
        """Start the bot."""
        log.info("cxdb bot starting...")
        self._write_pid()
        self._ensure_channel()

        signal.signal(signal.SIGTERM, self._shutdown)
        signal.signal(signal.SIGINT, self._shutdown)

        log.info(f"Listening on #{CHANNEL}")
        self.zulip.call_on_each_message(self._handle_message)

    def _shutdown(self, signum, frame):
        log.info(f"Shutdown signal {signum}")
        self.running = False
        self.cxdb.close()
        PID_FILE.unlink(missing_ok=True)
        sys.exit(0)

    def _write_pid(self):
        PID_FILE.parent.mkdir(parents=True, exist_ok=True)
        PID_FILE.write_text(str(os.getpid()))

    def _ensure_channel(self):
        """Create #cxdb channel if needed."""
        result = self.zulip.add_subscriptions(
            streams=[{
                "name": CHANNEL,
                "description": (
                    "cxdb - Conversation branching for Oracle-Cortex. "
                    "Fork sessions, A/B test prompts, score branches. "
                    "CTX-N links auto-resolve to cxdb contexts."
                ),
            }],
        )
        if result["result"] == "success":
            log.info(f"Subscribed to #{CHANNEL}")

    def _reply(self, msg: dict, content: str):
        """Reply in the same topic."""
        self.zulip.send_message({
            "type": "stream",
            "to": msg.get("display_recipient", CHANNEL),
            "topic": msg.get("subject", "general"),
            "content": content,
        })

    def _post(self, topic: str, content: str):
        """Post to a specific topic in #cxdb."""
        self.zulip.send_message({
            "type": "stream",
            "to": CHANNEL,
            "topic": topic,
            "content": content,
        })

    def _react(self, msg: dict, emoji: str = "eyes"):
        try:
            self.zulip.add_reaction({
                "message_id": msg["id"],
                "emoji_name": emoji,
            })
        except Exception:
            pass

    # ── Message routing ─────────────────────────────────────────

    def _handle_message(self, msg: dict):
        """Route incoming messages."""
        if msg.get("sender_email", "").startswith(BOT_EMAIL_PREFIX):
            return

        if msg.get("type") != "stream":
            return

        stream = msg.get("display_recipient", "")
        content = msg.get("content", "").strip()

        # Only handle messages in #cxdb or @-mentions elsewhere
        if stream != CHANNEL:
            if "@**cxdb Bot**" not in content and f"@**{BOT_EMAIL_PREFIX}**" not in content:
                return

        # Strip bot mention
        text = re.sub(r'@\*\*[^*]+\*\*\s*', '', content).strip().lower()
        text_raw = re.sub(r'@\*\*[^*]+\*\*\s*', '', content).strip()

        try:
            if text.startswith("sessions") or text.startswith("list"):
                self._handle_sessions(msg)
            elif text.startswith("show"):
                self._handle_show(msg, text_raw)
            elif text.startswith("fork"):
                self._handle_fork(msg, text_raw)
            elif text.startswith("compare"):
                self._handle_compare(msg, text_raw)
            elif text.startswith("score"):
                self._handle_score(msg, text_raw)
            elif text.startswith("search"):
                self._handle_search(msg, text_raw)
            elif text.startswith("record"):
                self._handle_record(msg, text_raw)
            elif text.startswith("help"):
                self._handle_help(msg)
            elif text:
                # Default: show help for unrecognized commands
                self._handle_help(msg)
        except CxdbError as e:
            self._reply(msg, f"**cxdb error** ({e.code}): {e.detail}")
        except Exception as e:
            log.exception(f"Error: {e}")
            self._reply(msg, f"**Error**: {e}")

    # ── Commands ────────────────────────────────────────────────

    def _handle_sessions(self, msg: dict):
        """List recent contexts."""
        self._react(msg)
        contexts = self.cxdb.list_contexts(limit=20)

        if not contexts:
            self._reply(msg, "No contexts yet. Use `record` or the Python API to create sessions.")
            return

        lines = ["**Recent Contexts**\n"]
        lines.append("| Context | Depth | Turns | Tag | Live |")
        lines.append("|---------|-------|-------|-----|------|")

        for ctx in contexts:
            ctx_id = ctx["context_id"]
            depth = ctx.get("head_depth", 0)
            tag = ctx.get("client_tag", "-")
            live = "yes" if ctx.get("is_live") else ""
            lines.append(f"| CTX-{ctx_id} | {depth} | {depth} | {tag} | {live} |")

        self._reply(msg, "\n".join(lines))

    def _handle_show(self, msg: dict, text: str):
        """Show turns from a context."""
        match = CTX_PATTERN.search(text)
        if not match:
            self._reply(msg, "Usage: `show CTX-<id>`")
            return

        self._react(msg)
        ctx_id = int(match.group(1))
        turns = self.cxdb.get_last(ctx_id, limit=30)

        if not turns:
            self._reply(msg, f"CTX-{ctx_id}: no turns found.")
            return

        lines = [f"**CTX-{ctx_id}** ({len(turns)} turns)\n"]
        for t in turns:
            data = t.data
            if data is None:
                lines.append(f"- Turn {t.turn_id} (depth {t.depth}): `{t.type_id}` [no payload]")
                continue

            type_short = t.type_id.split(".")[-1]

            if t.type_id == "com.oracle.agent.SessionMeta":
                agent = data.get(2, "?")
                trigger = data.get(4, "?")
                stream = data.get(5, "")
                topic = data.get(6, "")
                loc = f" in #{stream} > {topic}" if stream else ""
                lines.append(f"- **Session** `{data.get(1, '?')}` by `{agent}` (trigger: {trigger}{loc})")
            elif t.type_id == "com.oracle.agent.ToolCall":
                tool = data.get(1, "?")
                status = data.get(6, "ok")
                dur = data.get(5, 0)
                icon = "white_check_mark" if status == "ok" else "x"
                lines.append(f"- :{icon}: **{tool}** ({dur}ms) @ turn {t.turn_id}")
            else:
                role = data.get(1, "?")
                content_text = data.get(2, "")
                # Truncate long content
                if len(content_text) > 200:
                    content_text = content_text[:200] + "..."
                meta = data.get(4, {})
                reward = meta.get("reward") if isinstance(meta, dict) else None
                reward_str = f" | **reward: {reward}**" if reward else ""
                lines.append(f"- **[{role}]** (turn {t.turn_id}, depth {t.depth}{reward_str}): {content_text}")

        self._reply(msg, "\n".join(lines))

    def _handle_fork(self, msg: dict, text: str):
        """Fork a context at a specific turn."""
        # Parse: fork CTX-1:17 "reason text"
        match = FORK_PATTERN.search(text)
        if not match:
            self._reply(msg, 'Usage: `fork CTX-<id>:<turn_id> "description"`\nExample: `fork CTX-1:17 "Try TDD approach"`')
            return

        self._react(msg, "fork_and_knife")
        ctx_id = int(match.group(1))
        turn_id = int(match.group(2))

        # Extract description (everything after the CTX-N:M pattern)
        desc_match = re.search(r'CTX-\d+:\d+\s+(.+)', text, re.IGNORECASE)
        description = desc_match.group(1).strip().strip('"\'') if desc_match else f"Fork from CTX-{ctx_id} turn {turn_id}"

        # Create the fork
        fork = self.cxdb.fork(turn_id)
        new_topic = f"[CTX-{fork.context_id}] {description}"

        # Post in the new topic
        self._post(new_topic, (
            f"**Forked** from CTX-{ctx_id} at turn {turn_id}\n\n"
            f"Parent: #**{CHANNEL}>[CTX-{ctx_id}]**\n"
            f"Branch head: CTX-{fork.context_id} (depth {fork.head_depth})\n\n"
            f"Use `show CTX-{fork.context_id}` to see shared history.\n"
            f"Append turns with the Python API or `record CTX-{fork.context_id} <role> <content>`."
        ))

        # Notify in the original topic
        self._reply(msg, (
            f":fork_and_knife: **Forked** at turn {turn_id} → CTX-{fork.context_id}\n"
            f"New topic: #**{CHANNEL}>{new_topic}**"
        ))

    def _handle_compare(self, msg: dict, text: str):
        """Compare multiple branches side by side."""
        ctx_ids = [int(x) for x in CTX_PATTERN.findall(text)]
        if len(ctx_ids) < 2:
            self._reply(msg, "Usage: `compare CTX-<id> CTX-<id> [CTX-<id> ...]`")
            return

        self._react(msg)
        comparison = self.explorer.compare_branches(ctx_ids, limit=20)

        lines = [f"**Branch Comparison** ({len(ctx_ids)} branches)\n"]

        for ctx_id, turns in comparison.items():
            lines.append(f"### CTX-{ctx_id} ({len(turns)} turns)")

            # Find the divergence point
            conversation_turns = [
                t for t in turns
                if t["type_id"] == "com.oracle.conversation.Turn"
            ]

            # Show last 5 conversation turns
            for t in conversation_turns[-5:]:
                data = t["data"]
                if data:
                    role = data.get(1, "?")
                    content_text = str(data.get(2, ""))[:120]
                    meta = data.get(4, {})
                    reward = meta.get("reward") if isinstance(meta, dict) else None
                    if reward:
                        lines.append(f"- **[{role}]** {content_text} | **reward: {reward}**")
                    else:
                        lines.append(f"- **[{role}]** {content_text}")

            lines.append("")

        # Find shared turns
        all_turn_sets = [
            {t["turn_id"] for t in turns}
            for turns in comparison.values()
        ]
        if all_turn_sets:
            shared = set.intersection(*all_turn_sets)
            unique_per_branch = {
                ctx_id: {t["turn_id"] for t in turns} - shared
                for ctx_id, turns in comparison.items()
            }
            lines.append(f"**Shared turns**: {len(shared)} | " +
                         " | ".join(f"CTX-{cid} unique: {len(u)}" for cid, u in unique_per_branch.items()))

        self._reply(msg, "\n".join(lines))

    def _handle_score(self, msg: dict, text: str):
        """Score a branch with a reward signal."""
        # Parse: score CTX-7 0.85 "Clean fix"
        match = CTX_PATTERN.search(text)
        if not match:
            self._reply(msg, 'Usage: `score CTX-<id> <reward> "reason"`\nExample: `score CTX-7 0.85 "Clean fix, tests pass"`')
            return

        ctx_id = int(match.group(1))

        # Extract reward value
        reward_match = re.search(r'CTX-\d+\s+([\d.]+)', text)
        if not reward_match:
            self._reply(msg, "Missing reward value (0.0-1.0)")
            return

        reward = float(reward_match.group(1))

        # Extract reason
        reason_match = re.search(r'[\d.]+\s+(.+)', text[text.index(reward_match.group(1)):])
        reason = reason_match.group(1).strip().strip('"\'') if reason_match else ""

        self._react(msg, "star")
        self.explorer.score_branch(ctx_id, reward=reward, reason=reason)

        emoji = "star" if reward >= 0.8 else ("thumbs_up" if reward >= 0.5 else "thumbs_down")
        self._reply(msg, f":{emoji}: CTX-{ctx_id} scored **{reward}**" + (f": {reason}" if reason else ""))

    def _handle_search(self, msg: dict, text: str):
        """Search across all contexts for matching content."""
        query = text[6:].strip().strip('"\'')
        if not query:
            self._reply(msg, "Usage: `search <query>`")
            return

        self._react(msg)
        contexts = self.cxdb.list_contexts(limit=50)
        results = []

        for ctx in contexts:
            ctx_id = int(ctx["context_id"])
            try:
                turns = self.cxdb.get_last(ctx_id, limit=50)
                for t in turns:
                    data = t.data
                    if data is None:
                        continue
                    content_text = str(data.get(2, ""))
                    if query.lower() in content_text.lower():
                        results.append({
                            "ctx_id": ctx_id,
                            "turn_id": t.turn_id,
                            "depth": t.depth,
                            "role": data.get(1, "?"),
                            "snippet": content_text[:150],
                        })
            except Exception:
                continue

        if not results:
            self._reply(msg, f'No results for "{query}"')
            return

        lines = [f'**Search results for "{query}"** ({len(results)} matches)\n']
        for r in results[:15]:
            lines.append(
                f"- CTX-{r['ctx_id']} turn {r['turn_id']} [{r['role']}]: {r['snippet']}"
            )
        if len(results) > 15:
            lines.append(f"\n*...and {len(results) - 15} more*")

        self._reply(msg, "\n".join(lines))

    def _handle_record(self, msg: dict, text: str):
        """Record a turn to a context via Zulip."""
        # Parse: record CTX-7 assistant "Here's the fix..."
        match = CTX_PATTERN.search(text)
        if not match:
            self._reply(msg, 'Usage: `record CTX-<id> <role> <content>`\nExample: `record CTX-7 assistant "Here is the fix..."`')
            return

        ctx_id = int(match.group(1))

        # Extract role and content after CTX-N
        rest = re.sub(r'record\s+CTX-\d+\s+', '', text, flags=re.IGNORECASE).strip()
        parts = rest.split(None, 1)
        if len(parts) < 2:
            self._reply(msg, 'Usage: `record CTX-<id> <role> <content>`')
            return

        role = parts[0].strip()
        content_text = parts[1].strip().strip('"\'')

        self._react(msg, "pencil")
        turn = self.cxdb.append_turn(ctx_id, role=role, content=content_text)
        self._reply(msg, f":pencil: Turn {turn.turn_id} appended to CTX-{ctx_id} (depth {turn.depth})")

    def _handle_help(self, msg: dict):
        """Show help message."""
        self._reply(msg, """**cxdb Bot** - Conversation Branching

| Command | Description |
|---------|-------------|
| `sessions` | List recent contexts |
| `show CTX-<id>` | Show turns from a context |
| `fork CTX-<id>:<turn> "desc"` | Fork at a turn → new topic |
| `compare CTX-1 CTX-2 CTX-3` | Side-by-side branch comparison |
| `score CTX-<id> 0.85 "reason"` | Attach reward signal |
| `record CTX-<id> <role> <text>` | Append a turn |
| `search <query>` | Search across all contexts |

**Linking**: Any `CTX-N` in messages/topics auto-links to the context data.
**Topics**: Each context maps to a `[CTX-N] description` topic.
**Forks**: `fork` creates both a cxdb branch and a new Zulip topic with back-link.

Python API: `from cortex.cxdb_client import CxdbClient`
Docs: `/home/devuser/Oracle-Cortex/docs/cxdb-conversation-branching.md`""")


def main():
    bot = CxdbBot()
    bot.start()


if __name__ == "__main__":
    main()
