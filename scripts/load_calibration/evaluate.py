"""`eval`: check the rubric's bars on held-out walkthroughs and propose new values."""

import glob
import json
import os

from build import read_jsonl, read_rubric, write
from stats import auc, cutoff, fit_logistic, p_overloaded, weights_from


def labels_in(out):
    """Labels from blind batches and from any other source under `labels/`."""
    labels = {}
    paths = glob.glob(os.path.join(out, "batches", "*", "labels.jsonl"))
    paths += glob.glob(os.path.join(out, "labels", "*.jsonl"))
    for path in sorted(paths):
        for label in read_jsonl(path):
            labels[label["item"]] = label
    return labels


def features(row, names, budget):
    tau = max(0.0, row["reading_seconds"] / budget - 1)
    return [row["answers"].get(name, 0.0) for name in names] + [tau]


def verdict(ok):
    return "pass" if ok else "**FAIL**"


def evaluate(args):
    rubric = read_rubric(args.gewu, args.rubric)
    with open(os.path.join(args.out, "manifest.json")) as file:
        manifest = json.load(file)
    if manifest["rubric_version"] != rubric["version"]:
        raise SystemExit(f"items were scored with rubric v{manifest['rubric_version']}, "
                         f"not v{rubric['version']}; rebuild")
    held_out = {w["index"] for w in manifest["walkthroughs"] if w["held_out"]}
    items = {i["item"]: i for i in read_jsonl(os.path.join(args.out, "items.jsonl"))}
    jev = {j["item"]: j for j in read_jsonl(os.path.join(args.out, "jev.jsonl"))}
    labels = labels_in(args.out)
    missing = sorted(set(items) - set(labels))
    if missing:
        raise SystemExit(f"{len(missing)} items unlabelled, e.g. {missing[:3]}")
    bars, theta = rubric["bars"], rubric["equation"]["threshold"]
    budget = rubric["budget"]["chunk_seconds"]
    names = [q["name"] for q in rubric["question"]]
    every = sorted(items)
    y = {n: bool(labels[n]["overloaded"]) for n in every}
    out = [f"Rubric v{rubric['version']}; {len(every)} labelled items, "
           f"{sum(y.values())} overloaded; Jev about ${manifest['jev_usd']:.4f}.\n",
           "### Each question against its own label (whole sample)\n",
           "| question | n | yes | AUC | pass |", "|---|---|---|---|---|"]
    bar1 = True
    for name in names:
        pairs = [(jev[n]["answers"][name], bool(labels[n]["answers"][name]["yes"]))
                 for n in every
                 if name in jev[n]["answers"] and name in labels[n].get("answers", {})]
        yes = sum(answer for _, answer in pairs)
        value = auc([p for p, _ in pairs], [answer for _, answer in pairs])
        judged = yes >= bars["min_labels"] and len(pairs) - yes >= bars["min_labels"]
        ok = (value or 0) >= bars["question_auc"] if judged else None
        bar1 &= ok is not False
        shown = "–" if value is None else f"{value:.3f}"
        out.append(f"| {name} | {len(pairs)} | {yes} | {shown} | "
                   f"{'too few of one label' if ok is None else verdict(ok)} |")
    fit = [n for n in every if items[n]["walkthrough"] not in held_out]
    held = [n for n in every if items[n]["walkthrough"] in held_out]
    beta0, beta = fit_logistic([features(jev[n], names, budget) for n in fit],
                               [float(y[n]) for n in fit])
    weights = weights_from(beta0, beta, theta)
    truth = [y[n] for n in held]
    m0 = auc([jev[n]["reading_seconds"] for n in held], truth)
    m1 = auc([jev[n]["overloaded"] for n in held], truth)
    out += [f"\n### Held out ({len(held)} items, {sum(truth)} overloaded)\n",
            f"- Reading time alone: AUC {fmt(m0)}", f"- Rubric as it stands: AUC {fmt(m1)}"]
    proposal = [f"# Proposal from {len(every)} labels (rubric v{rubric['version']})\n",
                "Apply by hand, bump `version`, and record the data in `[provenance]`.\n"]
    if weights is None:
        out.append("- Fitted: β₀ ≥ 0, no boundary at L ≥ θ; the model bars fail.")
        m2 = accuracy = brier = None
    else:
        def score(n):
            x = features(jev[n], names, budget)
            return p_overloaded(x[:-1], weights[:-1], weights[-1] * x[-1], theta)
        p = [score(n) for n in held]
        m2 = auc(p, truth)
        accuracy = sum((q >= 0.5) == t for q, t in zip(p, truth)) / len(held)
        brier = sum((q - t) ** 2 for q, t in zip(p, truth)) / len(held)
        out.append(f"- Fitted on the other walkthroughs: AUC {fmt(m2)}, accuracy "
                   f"{accuracy:.3f}, Brier {brier:.3f}")
        proposal.append("```toml\n[equation]\ntime_weight = %.2f\n```\n" % weights[-1])
        proposal += [f"- `{name}`: weight {w:.2f}" for name, w in zip(names, weights)]
    best, curve = cutoff([jev[n]["reading_seconds"] for n in fit], [y[n] for n in fit],
                         bars["max_false_flags"])
    if best:
        t = best[0]
        caught = sum(jev[n]["reading_seconds"] > t for n in held if y[n])
        flagged = sum(jev[n]["reading_seconds"] > t for n in held if not y[n])
        out.append(f"- Budget {t} s (chosen on the fitted walkthroughs) catches {caught} of "
                   f"{sum(truth)} overloaded and flags {flagged} of {len(held) - sum(truth)} fine")
        proposal.append(f"\n```toml\n[budget]\nchunk_seconds = {t}\n```\n")
    proposal.append("\n| cutoff s | catches | flags fine | balanced |\n|---|---|---|---|")
    proposal += [f"| {t} | {c:.0%} | {f:.0%} | {b:.2f} |" for t, c, f, b in curve[::3]]
    bar2 = (m2 or 0) >= bars["model_auc"]
    bar3 = accuracy is not None and accuracy >= bars["accuracy"] and brier <= bars["brier"]
    bar4 = m2 is not None and m0 is not None and m2 >= m0 + bars["jev_margin"]
    bar5 = manifest["jev_usd"] <= bars["max_spend_usd"]
    out += ["\n| bar | pass |", "|---|---|",
            f"| every judged question AUC ≥ {bars['question_auc']} | {verdict(bar1)} |",
            f"| fitted AUC ≥ {bars['model_auc']} | {verdict(bar2)} |",
            f"| accuracy ≥ {bars['accuracy']}, Brier ≤ {bars['brier']} | {verdict(bar3)} |",
            f"| Jev beats reading time by {bars['jev_margin']} | {verdict(bar4)} |",
            f"| Jev ≤ ${bars['max_spend_usd']} | {verdict(bar5)} |"]
    write(os.path.join(args.out, "results.md"), "\n".join(out) + "\n")
    write(os.path.join(args.out, "proposal.md"), "\n".join(proposal) + "\n")
    print("\n".join(out))
    print(f"\nProposal written to {args.out}/proposal.md")


def fmt(value):
    return "–" if value is None else f"{value:.3f}"
