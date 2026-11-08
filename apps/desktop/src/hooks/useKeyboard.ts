import { useEffect, useRef } from "react";

import type { ReviewController } from "./useReviewApp";

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.isContentEditable) {
    return true;
  }
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

export function useKeyboard(app: ReviewController): void {
  const appRef = useRef(app);

  useEffect(() => {
    appRef.current = app;
  }, [app]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const current = appRef.current;
      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }

      const key = event.key;
      if (key === "Escape") {
        if (current.overlay) {
          event.preventDefault();
          current.closeOverlay();
        }
        return;
      }

      if (key === "?" || (key === "/" && event.shiftKey)) {
        if (isTypingTarget(event.target)) {
          return;
        }
        event.preventDefault();
        if (current.overlay === "help") {
          current.closeOverlay();
        } else if (current.overlay === null) {
          current.toggleHelp();
        }
        return;
      }

      if (isTypingTarget(event.target) || current.overlay) {
        return;
      }

      switch (key) {
        case "j":
          event.preventDefault();
          void current.movePage(1);
          break;
        case "k":
          event.preventDefault();
          void current.movePage(-1);
          break;
        case "o":
          event.preventDefault();
          void current.goto({ screen: "overview" });
          break;
        case "s":
          event.preventDefault();
          void current.goto({ screen: "summary" });
          break;
        case "r":
          event.preventDefault();
          void current.toggleReviewed();
          break;
        case "c":
          event.preventDefault();
          current.openFeedback("note");
          break;
        case "x":
          event.preventDefault();
          current.openFeedback("issue");
          break;
        default:
          break;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
