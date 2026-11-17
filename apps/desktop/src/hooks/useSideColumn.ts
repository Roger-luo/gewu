import { useEffect, useState, type RefObject } from "react";

/** Fired when the side column's width changes, so the layout re-measures. */
export const SIDE_WIDTH_EVENT = "gewu:side-width";

/** The narrowest reading column worth keeping a side column beside, in rem. */
const MIN_READING_REM = 36;

/**
 * Whether `main` is wide enough for a side column beside the reading column,
 * where the concept card and the quotes' notes go, like a blog's margin
 * notes. Below that they stay in the column. Without a way to measure (a
 * test DOM), it assumes a wide window.
 */
export function useSideColumn(main: RefObject<HTMLElement | null>): boolean {
  const [wide, setWide] = useState(true);

  useEffect(() => {
    const element = main.current;
    if (!element || typeof ResizeObserver === "undefined") {
      return;
    }
    const measure = () => {
      const style = getComputedStyle(element);
      const content =
        element.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      const root = getComputedStyle(document.documentElement);
      const rem = parseFloat(root.fontSize) || 16;
      const side = parseFloat(root.getPropertyValue("--context-width")) || 400;
      const gap = parseFloat(style.getPropertyValue("--side-gap-px")) || 1.5 * rem;
      setWide(content - side - gap >= MIN_READING_REM * rem);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    window.addEventListener(SIDE_WIDTH_EVENT, measure);
    return () => {
      observer.disconnect();
      window.removeEventListener(SIDE_WIDTH_EVENT, measure);
    };
  }, [main]);

  return wide;
}
