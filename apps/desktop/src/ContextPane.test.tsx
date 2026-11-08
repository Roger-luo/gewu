import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ContextPane } from "./components/ContextPane";

describe("context pane", () => {
  it("renders inline Markdown in prose and highlights the example", async () => {
    const { container } = render(
      <ContextPane
        language="typescript"
        context={{
          kind: "concept",
          definition: "`goto` turns a target into a screen change.",
          example: "const chunk = snapshot?.chunks[target.index];",
          relation: "The index comes from the newer snapshot `next`.",
        }}
      />,
    );

    expect(screen.getByText("goto").tagName).toBe("CODE");
    expect(screen.getByText("next").tagName).toBe("CODE");
    expect(container.textContent).not.toContain("`");
    await screen.findByText((_, element) => element?.classList.contains("shiki") ?? false);
  });
});
