from .reduce import fits


def deliver(author, stage, request, apply):
    """Apply a checkpoint, with at most two bounded validation repair calls."""
    current, current_stage = request, stage
    for attempt in range(3):
        if not fits(author, current_stage, current):
            raise ValueError("complete checkpoint request exceeds budget before provider call")
        draft = author.call(current_stage, current)
        try:
            apply(draft)
            return draft
        except ValueError as error:
            if attempt == 2:
                raise
            current = {"instructions": "Correct this rejected checkpoint. Return only the corrected JSON object.",
                       "original_request": request, "rejected_draft": draft, "validation_error": str(error)[:2048]}
            current_stage = "repair"
