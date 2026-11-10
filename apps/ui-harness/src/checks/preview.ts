import type { Page } from "playwright";
import type { CheckFinding } from "../types.js";

/**
 * Flags a preview that paints nothing, cuts off the drawing, or leaves a page
 * fill smaller than the marks. The callback stays free of nested functions so
 * the runner can send it to the page as written.
 */
export async function runPreviewChecks(
  page: Page,
  meta: { pageId: string; viewport: string },
): Promise<CheckFinding[]> {
  const found = await page.evaluate((): {
    key: "preview.blank" | "preview.clipped" | "preview.backdrop";
    selector: string;
    description: string;
  }[] => {
    const out: {
      key: "preview.blank" | "preview.clipped" | "preview.backdrop";
      selector: string;
      description: string;
    }[] = [];
    const asides = Array.from(document.querySelectorAll("aside.quote-preview"));
    for (const aside of asides) {
      const svg = aside.querySelector("svg");
      const img = aside.querySelector("img");
      let drawn = false;
      if (svg) {
        if (svg.querySelector("rect, line, polyline, polygon, circle, ellipse, image, text, use")) {
          drawn = true;
        }
        const nodes = Array.from(svg.querySelectorAll("[fill], [stroke]"));
        for (const node of nodes) {
          const paints = [node.getAttribute("fill"), node.getAttribute("stroke")];
          for (const paint of paints) {
            if (!paint) continue;
            const value = paint.trim().toLowerCase();
            if (
              value === "none" ||
              value === "transparent" ||
              value === "white" ||
              value === "#fff" ||
              value === "#ffffff"
            ) {
              continue;
            }
            drawn = true;
          }
        }
      } else if (img instanceof HTMLImageElement) {
        drawn = !img.complete || img.naturalWidth > 0;
      }
      const label = aside.getAttribute("aria-label") ?? "preview";
      if (!drawn) {
        out.push({
          key: "preview.blank",
          selector: "aside.quote-preview",
          description: `Rendered preview draws nothing: "${label}"`,
        });
      } else if (svg instanceof SVGElement) {
        const view = svg.getBoundingClientRect();
        let cutOff = false;
        const shapes = Array.from(
          svg.querySelectorAll("path, rect, circle, ellipse, line, polyline, polygon, image, use, text"),
        );
        for (const shape of shapes) {
          const box = shape.getBoundingClientRect();
          if (box.width < 1 && box.height < 1) continue;
          if (
            box.left < view.left - 2 ||
            box.top < view.top - 2 ||
            box.right > view.right + 2 ||
            box.bottom > view.bottom + 2
          ) {
            cutOff = true;
          }
        }
        let node = svg.parentElement;
        for (let depth = 0; node && depth < 8; depth += 1, node = node.parentElement) {
          const style = getComputedStyle(node);
          const mode = `${style.overflow} ${style.overflowX} ${style.overflowY}`;
          if (!mode.includes("hidden") && !mode.includes("clip")) continue;
          const pane = node.getBoundingClientRect();
          const cut =
            Math.max(0, pane.left - view.left) +
            Math.max(0, pane.top - view.top) +
            Math.max(0, view.right - pane.right) +
            Math.max(0, view.bottom - pane.bottom);
          if (cut > 8) cutOff = true;
        }
        let partialPaper = false;
        const paper: DOMRect[] = [];
        const ink: DOMRect[] = [];
        const painted = Array.from(svg.querySelectorAll("[fill], [stroke]"));
        for (const el of painted) {
          const fill = (el.getAttribute("fill") ?? "").trim().toLowerCase();
          const stroke = (el.getAttribute("stroke") ?? "").trim().toLowerCase();
          const box = el.getBoundingClientRect();
          if (box.width < 1 && box.height < 1) continue;
          const paperFill = fill === "white" || fill === "#fff" || fill === "#ffffff";
          if (paperFill) paper.push(box);
          const inkFill = fill !== "" && fill !== "none" && fill !== "transparent" && !paperFill;
          const inkStroke =
            stroke !== "" && stroke !== "none" && stroke !== "transparent" && stroke !== "white" && stroke !== "#fff" && stroke !== "#ffffff";
          if (inkFill || inkStroke) ink.push(box);
        }
        if (paper.length > 0) {
          for (const box of ink) {
            let covered = false;
            for (const ground of paper) {
              if (
                ground.left <= box.left + 2 &&
                ground.top <= box.top + 2 &&
                ground.right >= box.right - 2 &&
                ground.bottom >= box.bottom - 2
              ) {
                covered = true;
              }
            }
            if (!covered) partialPaper = true;
          }
        }
        if (cutOff) {
          out.push({
            key: "preview.clipped",
            selector: "aside.quote-preview",
            description: `Rendered preview is cut off: "${label}" shows only part of the drawing`,
          });
        } else if (partialPaper) {
          out.push({
            key: "preview.backdrop",
            selector: "aside.quote-preview",
            description: `Rendered preview has a page fill smaller than the drawing: "${label}"`,
          });
        }
      }
      if (out.length >= 25) break;
    }
    return out;
  });

  return found.map((item) => ({
    key: item.key,
    page: meta.pageId,
    viewport: meta.viewport,
    selector: item.selector,
    description: item.description,
  }));
}
