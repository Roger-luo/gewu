"""Every finding across runs, until a later run shows it gone.

A finding is closed in two steps. Someone marks it fixed and names the test or
commit that fixed it; the next run of the same corpus under the same rubric
then verifies it by not reproducing it, or reopens it as a regression.
"""
import json
from pathlib import Path
import time

from gewu_driver.storage import save

OPEN, FIXED, REJECTED, VERIFIED, REGRESSED = "open", "fixed", "rejected", "verified", "regressed"


class Ledger:
    def __init__(self, path):
        self.path = Path(path)
        self.data = (json.loads(self.path.read_text()) if self.path.exists()
                     else {"runs": [], "findings": {}})

    def save(self):
        save(self.path, self.data)

    def record(self, run, result):
        """Adds one scored run and moves every affected finding's status."""
        corpus = result["corpus"]
        run_id = f"{corpus}-{len(self.data['runs']) + 1}"
        previous = [entry for entry in self.data["runs"] if entry["corpus"] == corpus]
        self.data["runs"].append({
            "id": run_id, "corpus": corpus, "at_unix": int(time.time()), "passes": result["passes"],
            "binary_sha256": run["binary_sha256"], "gewu_source": run["gewu_source"],
            "rubric_version": result["rubric_version"], "gates": result["gates"],
            "minimality": result["minimality"], "labels": result["labels"],
            "dimensions": result["dimensions"], "tui": result.get("tui", {"status": "unassessed"}),
            "cold_reader": result.get("cold_reader", {"status": "unassessed"}),
            "findings": len(result["findings"])})
        current = {finding["key"]: finding for finding in result["findings"]}
        for identifier, finding in current.items():
            entry = self.data["findings"].get(identifier)
            if entry is None:
                self.data["findings"][identifier] = {**finding, "status": OPEN, "first_run": run_id,
                                                     "last_run": run_id, "history": []}
                continue
            if entry["status"] in (FIXED, VERIFIED):
                self.move(entry, REGRESSED, f"reproduced in {run_id}")
            entry["last_run"] = run_id
        for identifier, entry in self.data["findings"].items():
            if entry["corpus"] == corpus and identifier not in current and entry["status"] == FIXED:
                self.move(entry, VERIFIED, f"not reproduced in {run_id}")
        self.save()
        return {"run": run_id, "delta": self.delta(previous[-1] if previous else None, self.data["runs"][-1])}

    @staticmethod
    def delta(before, after):
        if before is None or before["rubric_version"] != after["rubric_version"]:
            return None
        return {"minimality": round(after["minimality"] - before["minimality"], 3),
                "labels": {name: after["labels"][name] - before["labels"][name] for name in after["labels"]},
                "dimensions": {name: after["dimensions"][name] - before["dimensions"].get(name, 0)
                               for name in after["dimensions"]},
                "tui": {name: after["tui"]["dimensions"][name] - before["tui"]["dimensions"][name]
                        for name in after.get("tui", {}).get("dimensions", {})
                        if name in before.get("tui", {}).get("dimensions", {})},
                "cold_reader": (round(after["cold_reader"]["accuracy"] - before["cold_reader"]["accuracy"], 3)
                                if "accuracy" in after.get("cold_reader", {})
                                and "accuracy" in before.get("cold_reader", {}) else None),
                "findings": after["findings"] - before["findings"]}

    def resolve(self, identifier, status, note):
        if status not in (FIXED, REJECTED):
            raise ValueError("resolve a finding as fixed or rejected")
        if not note.strip():
            raise ValueError("name the test or commit that fixed it, or why it is mistaken")
        entry = self.data["findings"].get(identifier)
        if entry is None:
            raise ValueError(f"unknown finding {identifier}")
        self.move(entry, status, note)
        self.save()

    def unresolved(self):
        return [{"key": identifier, **entry} for identifier, entry in self.data["findings"].items()
                if entry["status"] in (OPEN, REGRESSED)]

    @staticmethod
    def move(entry, status, note):
        entry["history"].append({"from": entry["status"], "to": status, "note": note})
        entry["status"] = status
