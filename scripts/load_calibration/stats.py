"""The arithmetic of calibration: ranking, fitting, and the load equation."""

import math


def auc(scores, labels):
    """Probability a random positive outscores a random negative; ties count half."""
    positives = [s for s, y in zip(scores, labels) if y]
    negatives = [s for s, y in zip(scores, labels) if not y]
    if not positives or not negatives:
        return None
    wins = sum((p > n) + 0.5 * (p == n) for p in positives for n in negatives)
    return wins / (len(positives) * len(negatives))


def fit_logistic(rows, labels, penalty=1.0, steps=20000, rate=0.5):
    """Intercept and coefficients by gradient descent; L2 on coefficients only."""
    width = len(rows[0])
    beta0, beta = 0.0, [0.0] * width
    n = len(rows)
    for _ in range(steps):
        g0, g = 0.0, [0.0] * width
        for x, y in zip(rows, labels):
            z = beta0 + sum(b * v for b, v in zip(beta, x))
            p = 1 / (1 + math.exp(-max(-30, min(30, z))))
            g0 += p - y
            for k in range(width):
                g[k] += (p - y) * x[k]
        beta0 -= rate * g0 / n
        beta = [b - rate * (gk / n + penalty * b / n) for b, gk in zip(beta, g)]
    return beta0, beta


def weights_from(beta0, beta, threshold=1.0):
    """The fitted boundary as `L ≥ θ`: w = θ·max(0, β)/−β₀. None when β₀ ≥ 0."""
    if beta0 >= 0:
        return None
    return [threshold * max(0.0, b) / -beta0 for b in beta]


def p_overloaded(ps, weights, time_load, threshold):
    """P(Σ w·X + time_load ≥ θ) over independent X ~ Bernoulli(p), exactly."""
    total = 0.0
    for outcome in range(1 << len(ps)):
        probability, load = 1.0, time_load
        for bit, (p, w) in enumerate(zip(ps, weights)):
            if outcome >> bit & 1:
                probability *= p
                load += w
            else:
                probability *= 1 - p
        if load + 1e-9 >= threshold:
            total += probability
    return total


def allocate(counts, total, minimum):
    """Items per walkthrough: proportional, at least `minimum`, summing to `total`."""
    whole = sum(counts.values())
    share = {key: min(n, max(minimum, round(total * n / whole))) for key, n in counts.items()}
    while sum(share.values()) > total:
        key = max(share, key=lambda k: share[k])
        share[key] -= 1
    while sum(share.values()) < total and any(share[k] < counts[k] for k in counts):
        key = max((k for k in counts if share[k] < counts[k]), key=lambda k: counts[k] - share[k])
        share[key] += 1
    return share


def cutoff(seconds, labels, max_false_flags):
    """The reading-time cutoff with the best balanced accuracy among those that
    flag at most `max_false_flags` of the chunks labelled fine, with its curve."""
    positives = [s for s, y in zip(seconds, labels) if y]
    negatives = [s for s, y in zip(seconds, labels) if not y]
    curve = []
    for t in range(30, 1201, 10):
        caught = sum(s > t for s in positives) / len(positives) if positives else 0.0
        flagged = sum(s > t for s in negatives) / len(negatives) if negatives else 0.0
        curve.append((t, caught, flagged, (caught + 1 - flagged) / 2))
    allowed = [point for point in curve if point[2] <= max_false_flags]
    best = max(allowed, key=lambda point: (point[3], -point[0])) if allowed else None
    return best, curve
