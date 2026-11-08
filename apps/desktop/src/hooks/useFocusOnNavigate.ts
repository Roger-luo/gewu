import { useEffect, useRef } from "react";

import type { Screen } from "../protocol";

/**
 * Move focus to the new screen's title after navigation so assistive tech
 * announces the page and the previously activated control stops looking
 * current. The first render keeps the browser's initial focus.
 */
export function useFocusOnNavigate(screen: Screen, chunkIndex: number | null) {
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    const main = document.querySelector<HTMLElement>("main[data-gewu-screen]");
    main?.scrollTo?.({ top: 0 });
    main?.querySelector<HTMLElement>(".screen-title")?.focus({ preventScroll: true });
  }, [screen, chunkIndex]);
}
