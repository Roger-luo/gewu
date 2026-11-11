#!/usr/bin/env python3
"""A stand-in ACP agent for tests and UI journeys: no model is called.

It speaks ACP protocol v1 over stdio and, on each prompt, drives the Gewu
harness the way a real agent would: through the client's terminal and file
methods only. Each batch expands one work item, so a review with several
changed files needs several batches.

FAKE_AGENT_MODE selects other behavior:
  harness (default)  write a valid walkthrough, one work item per batch
  probe              exercise the policy: one allowed write, one refused
                     write, one refused command, then end the turn
  meddle             edit a project file directly, then end the turn
  fail               exit with an error before answering the prompt
FAKE_AGENT_LOGIN makes session/new need `authenticate` first, as Cursor's
agent does: `required` accepts the advertised method, `refused` turns it
down as a signed-out CLI would.
FAKE_AGENT_DELAY is the pause between steps in seconds (default 0.2), so a
UI can be watched while a batch runs.
"""
import json
import os
import re
import sys
import time

MODE = os.environ.get("FAKE_AGENT_MODE", "harness")
DELAY = float(os.environ.get("FAKE_AGENT_DELAY", "0.2"))
LOGIN = os.environ.get("FAKE_AGENT_LOGIN", "")
SIGN_IN = "Authentication required. Please run 'fake login' first."


class Cancelled(Exception):
    pass


class Agent:
    def __init__(self):
        self.next_id = 1000
        self.session = None
        self.cwd = None
        self.cancelled = False

    def send(self, message):
        message["jsonrpc"] = "2.0"
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()

    def read(self):
        line = sys.stdin.readline()
        if not line:
            sys.exit(0)
        return json.loads(line)

    def request(self, method, params):
        """Send a request to the client and wait for its response."""
        self.next_id += 1
        request_id = self.next_id
        self.send({"id": request_id, "method": method, "params": params})
        while True:
            message = self.read()
            if message.get("method") == "session/cancel":
                self.cancelled = True
                continue
            if message.get("id") == request_id and "method" not in message:
                return message

    def update(self, update):
        self.send({"method": "session/update",
                   "params": {"sessionId": self.session, "update": update}})

    def say(self, text):
        self.update({"sessionUpdate": "agent_message_chunk",
                     "content": {"type": "text", "text": text}})

    def pause(self):
        time.sleep(DELAY)
        if self.cancelled:
            raise Cancelled()

    def run(self, words):
        """Run a command in a client terminal; returns (exit code, output)."""
        self.update({"sessionUpdate": "tool_call", "toolCallId": f"t{self.next_id}",
                     "title": " ".join(words), "kind": "execute", "status": "pending"})
        created = self.request("terminal/create", {"sessionId": self.session,
                                                   "command": words[0], "args": words[1:]})
        if "error" in created:
            return None, created["error"]["message"]
        terminal = created["result"]["terminalId"]
        exit_status = self.request("terminal/wait_for_exit",
                                   {"sessionId": self.session, "terminalId": terminal})["result"]
        output = self.request("terminal/output",
                              {"sessionId": self.session, "terminalId": terminal})["result"]["output"]
        self.request("terminal/release", {"sessionId": self.session, "terminalId": terminal})
        return exit_status.get("exitCode"), output

    def read_file(self, path):
        response = self.request("fs/read_text_file",
                                {"sessionId": self.session, "path": os.path.join(self.cwd, path)})
        return response["result"]["content"]

    def write_file(self, path, value):
        text = value if isinstance(value, str) else json.dumps(value, indent=2)
        self.update({"sessionUpdate": "tool_call", "toolCallId": f"w{self.next_id}",
                     "title": f"Write {path}", "kind": "edit", "status": "pending",
                     "locations": [{"path": os.path.join(self.cwd, path)}]})
        return self.request("fs/write_text_file", {"sessionId": self.session,
                                                    "path": os.path.join(self.cwd, path),
                                                    "content": text})

    def prompt(self, text):
        match = re.search(r"The Gewu binary is `([^`]+)`", text)
        gewu = match.group(1) if match else "gewu"
        if MODE == "fail":
            sys.stderr.write("simulated failure while writing the walkthrough\n")
            sys.exit(3)
        if MODE == "probe":
            self.say("Probing the policy.")
            self.write_file(".gewu/harness/probe.txt", "allowed")
            self.write_file("outside.txt", "refused")
            self.run(["rm", "-rf", "."])
            return
        if MODE == "meddle":
            with open(os.path.join(self.cwd, "MEDDLED.txt"), "w") as handle:
                handle.write("the agent edited the project\n")
            self.say("I edited the project.")
            return
        self.harness(gewu)

    def harness(self, gewu):
        code, status = self.run([gewu, "harness", "status"])
        self.pause()
        manifest = json.loads(self.read_file(".gewu/review-input.json"))
        changes = manifest["changes"]
        source = manifest["source"]
        if status.startswith("# Gewu harness: investigate"):
            self.say(f"Investigating {len(changes)} changed file(s).\n")
            self.write_file(".gewu/harness/investigation.json", {
                "schema_version": 1, "source": source,
                "intent": "Walk through the selected changes one file at a time.",
                "changes": [f"Changes {change['path']}." for change in changes[:5]],
                "concepts": [],
                "findings": [{"change_ids": [change["id"]],
                              "summary": f"{change['path']} changes.",
                              "evidence": [change["path"]]} for change in changes],
            })
            self.pause()
            code, status = self.run([gewu, "harness", "next"])
            self.pause()
        if status.startswith("# Gewu harness: plan"):
            self.say("Planning one step per file.\n")
            items = [{"id": f"item-{index + 1}", "title": f"Review `{change['path']}`",
                      "purpose": f"Read the edits to {change['path']}.",
                      "covers": [{"change_id": change["id"], "start": 1,
                                  "end": change["body_lines"]}],
                      "expanded": False}
                     for index, change in enumerate(changes)]
            self.write_file(".gewu/harness/worklist.json",
                            {"schema_version": 1, "source": source, "items": items})
            self.pause()
            code, status = self.run([gewu, "harness", "next"])
            self.pause()
        if status.startswith("# Gewu harness: expand"):
            worklist = json.loads(self.read_file(".gewu/harness/worklist.json"))
            item = next(item for item in worklist["items"] if not item["expanded"])
            self.say(f"Writing the step for {item['title']}.\n")
            chunk = {"id": f"chunk-{item['id']}", "title": item["title"],
                     "summary": item["purpose"], "kind": "other",
                     "content": [{"type": "changes", "quotes": item["covers"],
                                  "presentation": "diff"}]}
            self.write_file(".gewu/harness/expansion.json",
                            {"schema_version": 1, "source": source,
                             "item_id": item["id"], "chunks": [chunk]})
            self.pause()
            code, status = self.run([gewu, "harness", "next"])
        elif status.startswith("# Gewu harness: proofread"):
            # Started fresh at a proofread: the batch is someone else's, so
            # score it. An agent that just wrote a batch stops instead.
            listed = status.split("Check every claim", 1)[0]
            ids = re.findall(r'^\s*"id": "([^"]+)"', listed, re.M)
            self.say(f"Proofreading {len(ids)} step(s).\n")
            scores = {name: 3 for name in ("grounded", "change_framing", "rationale",
                                           "representation_contrast", "minimality",
                                           "connection")}
            self.write_file(".gewu/harness/proofread.json", {
                "schema_version": 1, "source": source,
                "chunks": [{"id": id, "scores": scores, "problems": []} for id in ids]})
            self.pause()
            code, status = self.run([gewu, "harness", "next"])
        self.say("Stopped: " + status.splitlines()[0] if status else "Stopped.")

    def serve(self):
        signed_in = not LOGIN
        while True:
            message = self.read()
            method = message.get("method")
            if method == "initialize":
                self.send({"id": message["id"], "result": {
                    "protocolVersion": 1, "agentCapabilities": {},
                    "agentInfo": {"name": "fake-acp-agent", "version": "1.0.0"},
                    "authMethods": [{"id": "fake_login", "name": "Fake Login"}] if LOGIN else []}})
            elif method == "authenticate":
                if LOGIN == "required" and message["params"]["methodId"] == "fake_login":
                    signed_in = True
                    self.send({"id": message["id"], "result": {}})
                else:
                    self.send({"id": message["id"], "error": {
                        "code": -32000, "message": "Authentication required",
                        "data": {"message": SIGN_IN}}})
            elif method == "session/new" and not signed_in:
                self.send({"id": message["id"], "error": {
                    "code": -32000, "message": "Authentication required",
                    "data": {"message": SIGN_IN}}})
            elif method == "session/new":
                self.cwd = message["params"]["cwd"]
                self.session = "fake-session"
                self.send({"id": message["id"], "result": {"sessionId": self.session}})
            elif method == "session/prompt":
                text = "".join(block.get("text", "")
                               for block in message["params"]["prompt"])
                self.cancelled = False
                try:
                    self.prompt(text)
                    reason = "end_turn"
                except Cancelled:
                    reason = "cancelled"
                self.send({"id": message["id"], "result": {"stopReason": reason}})
            elif method == "session/cancel":
                self.cancelled = True
            elif "id" in message and method:
                self.send({"id": message["id"],
                           "error": {"code": -32601, "message": f"unknown method {method}"}})


if __name__ == "__main__":
    Agent().serve()
