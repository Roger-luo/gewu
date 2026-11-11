import { fireEvent, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { renderApp, waitForReady } from "../../test/renderApp";

async function openStep() {
  renderApp("explainers");
  await waitForReady();
  await userEvent.setup().click(screen.getByRole("button", { name: /start guided review/i }));
  await waitForReady();
  return screen.getByRole("main");
}

describe("explainers", () => {
  it("quotes the claim the step serves and how", async () => {
    const main = await openStep();
    const claim = within(main).getByRole("figure", { name: "Claim this step serves" });
    expect(claim).toHaveTextContent("use 61–65% less peak memory");
    expect(claim).toHaveTextContent("Whole change, measured82-spin ladder");
    expect(claim).toHaveTextContent("This step's partabout 36 MiB of the 573 MiB drop");
  });

  it("draws each entry to scale and states the ratio", async () => {
    const main = await openStep();
    const layout = within(main).getByRole("figure", { name: "One index entry" });
    expect(layout).toHaveTextContent("41 B");
    expect(layout).toHaveTextContent("5 B");
    expect(layout).toHaveTextContent("ctrl 1 B");
    expect(layout).toHaveTextContent("After is 8.2× smaller than before.");
  });

  it("recomputes the cost when the reader drags the parameter", async () => {
    const main = await openStep();
    const model = within(main).getByRole("figure", { name: "Index memory at basis size B" });
    const results = within(model).getByRole("list", { name: "Results" });
    expect(results).toHaveTextContent("41.0 MiB");
    expect(results).toHaveTextContent("5.0 MiB");

    const slider = within(model).getByRole("slider", { name: "Strings in the basis (B)" });
    expect(slider).toHaveAttribute("aria-valuetext", "2^19 = 524,288");
    // 2^15 … 2^21 are positions 0 … 6; jsdom does not move a range on arrow keys.
    fireEvent.change(slider, { target: { value: "5" } });
    expect(slider).toHaveAttribute("aria-valuetext", "2^20 = 1,048,576");
    expect(results).toHaveTextContent("82.0 MiB");
    expect(model).toHaveTextContent("88% less (8.2× smaller)");
    expect(within(model).getByText("How it is computed")).toBeInTheDocument();

    expect(slider).toHaveAttribute("aria-valuenow", "1048576");
    await userEvent.setup().click(within(model).getByRole("button", { name: "2^17" }));
    expect(slider).toHaveAttribute("aria-valuetext", "2^17 = 131,072");
    expect(within(model).getByRole("button", { name: "2^17" })).toHaveAttribute("aria-pressed", "true");
  });

  it("plays the record from before to after, and replays on demand", async () => {
    const main = await openStep();
    const layout = within(main).getByRole("figure", { name: "One index entry" });
    const show = within(layout).getByRole("group", { name: "Show record" });
    expect(within(show).getByRole("button", { name: "Before" })).toHaveAttribute("aria-pressed", "true");
    await vi.waitFor(
      () => expect(within(show).getByRole("button", { name: "After" })).toHaveAttribute("aria-pressed", "true"),
      { timeout: 3000 },
    );
    await vi.waitFor(() => expect(layout).toHaveTextContent("After: 5 bytes, u32, ctrl"));

    await userEvent.setup().click(within(show).getByRole("button", { name: "Before" }));
    expect(layout).toHaveTextContent("Before: 41 bytes, Word<C> copy, u32, pad, ctrl");
  });

  it("sweeps the parameter so the bars grow in front of the reader", async () => {
    const main = await openStep();
    const model = within(main).getByRole("figure", { name: "Index memory at basis size B" });
    const play = within(model).getByRole("button", { name: /play through strings in the basis/i });
    await userEvent.setup().click(play);
    const slider = within(model).getByRole("slider", { name: "Strings in the basis (B)" });
    expect(slider).toHaveAttribute("aria-valuetext", "2^15 = 32,768");
    expect(within(model).getByRole("button", { name: /pause/i })).toHaveAttribute("aria-pressed", "true");
  });
});
