import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { quote, line } from "../mock/builders";
import { AddedCodeQuote } from "./AddedCodeQuote";

describe("added code", () => {
  it("shows a typst color and a rendered example beside the source", () => {
    render(
      <AddedCodeQuote
        quote={quote({
          change_id: "palette",
          path: "diagrams/lib/illustration.typ",
          start: 1,
          end: 1,
          language: "typst",
          lines: [line("added", '#let ink = rgb("243746")', null, 20)],
          preview: {
            label: "What these colors look like",
            media: "image/svg+xml",
            data: '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect fill="#243746"/></svg>',
          },
        })}
        selectedLines={new Set()}
        onSelectLine={() => undefined}
      />,
    );

    expect(screen.getByRole("img", { name: "Color #243746" })).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "What these colors look like" })).toBeInTheDocument();
  });
});
