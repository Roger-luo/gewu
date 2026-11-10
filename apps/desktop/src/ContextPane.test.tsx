import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  it("shows both definitions at once and marks the field the new one dropped", async () => {
    localStorage.clear();
    const user = userEvent.setup();
    render(
      <ContextPane
        language="rust"
        context={{
          kind: "refactor",
          before:
            "pub struct PauliWord<A: PauliStorage, S, const R: bool> { pub xbits: BitArray<A>, pub zbits: BitArray<A>, nqubits: usize, hash_cache: u64, _phantom: PhantomData<S> }",
          after:
            "pub struct Word<const C: usize = W_CHUNKS> { pub xbits: BitArray<Chunk, C>, pub zbits: BitArray<Chunk, C> }",
          reason:
            "Drops `nqubits`, `hash_cache`, and `_phantom` so `Word<2>` shrinks from 48 to 32 bytes.",
        }}
      />,
    );

    expect(screen.queryByText(/before \/ after signatures/i)).not.toBeInTheDocument();
    expect(screen.getByText("Before")).toBeVisible();
    expect(screen.getByText("After")).toBeVisible();
    const before = screen.getByText("Before").parentElement;
    expect(before?.textContent).toContain("pub struct PauliWord<A: PauliStorage, S, const R: bool> {");
    expect(before?.textContent).toContain("\n    nqubits: usize,");
    const marked = await screen.findAllByTitle("Dropped");
    expect(marked.map((mark) => mark.textContent)).toEqual(
      expect.arrayContaining(["nqubits", "hash_cache", "_phantom"]),
    );
    expect(screen.getByText("After").parentElement?.querySelector(".dropped-field")).toBeNull();

    const handle = screen.getByRole("separator", { name: "Context pane width" });
    expect(handle).toHaveAttribute("aria-valuenow", "560");
    handle.focus();
    await user.keyboard("{ArrowLeft}");
    expect(handle).toHaveAttribute("aria-valuenow", "576");
    expect(document.documentElement.style.getPropertyValue("--context-width")).toBe("576px");
  });
});
