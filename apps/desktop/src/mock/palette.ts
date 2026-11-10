import { changes, chunk, line, narrative, quote, snapshot } from "./builders";
import type { Snapshot } from "../protocol";

const sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="220" height="184" viewBox="0 0 220 184">
  <rect x="12" y="12" width="16" height="16" rx="2" fill="#243746"/>
  <text x="38" y="25" font-size="13" fill="#243746">ink</text>
  <rect x="12" y="40" width="16" height="16" rx="2" fill="#526575"/>
  <text x="38" y="53" font-size="13" fill="#243746">muted</text>
  <rect x="12" y="68" width="16" height="16" rx="2" fill="#d2dce3"/>
  <text x="38" y="81" font-size="13" fill="#243746">quiet</text>
  <rect x="12" y="96" width="16" height="16" rx="2" fill="#0072b2"/>
  <text x="38" y="109" font-size="13" fill="#243746">accent</text>
  <rect x="12" y="124" width="16" height="16" rx="2" fill="#009e73"/>
  <text x="38" y="137" font-size="13" fill="#243746">data</text>
  <rect x="12" y="152" width="16" height="16" rx="2" fill="#b86a00"/>
  <text x="38" y="165" font-size="13" fill="#243746">feedback</text>
</svg>`;

/** `?mock=palette` shows a Typst color library with each color and a rendered example. */
export function paletteSnapshot(): Snapshot {
  return snapshot({
    review_id: "demo-palette",
    source: { base: "4b825dc642cb6eb9a060e54bf8d69288fbee4904", head: "main" },
    overview: {
      intent: "Name the inks a diagram uses, so a reviewer can see each one.",
      changes: ["Bind a neutral palette and three Okabe–Ito hues."],
      concepts: [],
    },
    chunks: [
      chunk({
        id: "palette",
        index: 0,
        title: "Diagram colors",
        summary: "Each binding is a color a later diagram paints with.",
        kind: "concept",
        blocks: [
          narrative("These names are the only colors the diagrams use."),
          changes("added_code", [
            quote({
              change_id: "illustration",
              path: "diagrams/lib/illustration.typ",
              start: 17,
              end: 27,
              language: "typst",
              lines: [
                line("added", "// Neutral palette.", null, 19),
                line("added", '#let ink = rgb("243746")', null, 20),
                line("added", '#let muted = rgb("526575")', null, 21),
                line("added", '#let quiet = rgb("D2DCE3")', null, 22),
                line("added", "// Semantic hues (Okabe–Ito derived).", null, 24),
                line("added", '#let accent = rgb("0072B2")', null, 25),
                line("added", '#let data = rgb("009E73")', null, 26),
                line("added", '#let feedback = rgb("B86A00")', null, 27),
              ],
              preview: {
                label: "What these colors look like",
                media: "image/svg+xml",
                data: sheet,
              },
            }),
          ]),
        ],
      }),
    ],
  });
}
