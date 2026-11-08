import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DiagramPlot } from "./components/DiagramPlot";

const mermaid = vi.hoisted(() => ({
  initialize: vi.fn(),
  parse: vi.fn(async (text: string) => (text.startsWith("flowchart") ? { diagramType: "flowchart" } : false)),
  render: vi.fn(async () => ({ svg: '<svg role="img"><g><text>drawn</text></g></svg>' })),
}));

vi.mock("mermaid", () => ({ default: mermaid }));

describe("diagram plot", () => {
  it("shows text that is not Mermaid as source instead of an error graphic", async () => {
    render(<DiagramPlot source={"mise.toml + build.sh\n   |\n   +--> diagrams/"} label="Diagram" />);

    expect(await screen.findByText(/shown as text because this is not a mermaid diagram/i)).toBeInTheDocument();
    expect(screen.getByText(/\+--> diagrams\//)).toBeInTheDocument();
    expect(mermaid.render).not.toHaveBeenCalled();
    expect(mermaid.initialize).toHaveBeenCalledWith(expect.objectContaining({ suppressErrorRendering: true }));
  });

  it("draws a Mermaid diagram", async () => {
    const { container } = render(
      <DiagramPlot source={"flowchart LR\n  a --> b"} label="Diagram of a, b" />,
    );

    expect(await screen.findByRole("img", { name: "Diagram of a, b" })).toBeInTheDocument();
    expect(container.querySelector(".illustration-plot svg")).not.toBeNull();
    expect(container.querySelector("[data-gewu-pending]")).toBeNull();
    // HTML labels live in a foreignObject the sanitizer removes, leaving empty boxes.
    expect(mermaid.initialize).toHaveBeenCalledWith(
      expect.objectContaining({ htmlLabels: false, flowchart: { htmlLabels: false } }),
    );
  });
});
