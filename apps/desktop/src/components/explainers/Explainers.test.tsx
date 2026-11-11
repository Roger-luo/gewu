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
    expect(claim).toHaveTextContent("This step's partabout 52 MiB of the 573 MiB drop");
  });

  it("draws each entry to scale and states the ratio", async () => {
    const main = await openStep();
    const layout = within(main).getByRole("figure", { name: "One index entry" });
    expect(layout).toHaveTextContent("57 B");
    expect(layout).toHaveTextContent("5 B");
    expect(layout).toHaveTextContent("ctrl 1 B");
    expect(layout).toHaveTextContent("After is 11.4× smaller than before.");
  });

  it("recomputes the cost when the reader drags the parameter", async () => {
    const main = await openStep();
    const model = within(main).getByRole("figure", { name: "Index memory at basis size B" });
    const results = within(model).getByRole("list", { name: "Results" });
    expect(results).toHaveTextContent("57.0 MiB");
    expect(results).toHaveTextContent("5.0 MiB");

    const slider = within(model).getByRole("slider", { name: "Strings in the basis (B)" });
    expect(slider).toHaveAttribute("aria-valuetext", "2^19 = 524,288");
    // 2^15 … 2^21 are positions 0 … 6; jsdom does not move a range on arrow keys.
    fireEvent.change(slider, { target: { value: "5" } });
    expect(slider).toHaveAttribute("aria-valuetext", "2^20 = 1,048,576");
    expect(results).toHaveTextContent("114.0 MiB");
    expect(model).toHaveTextContent("91% less (11.4× smaller)");
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
    expect(layout).toHaveTextContent("Before: 57 bytes, xbits, zbits, nqubits, hash_cache, u32, pad, ctrl");
  });

  it("sweeps the parameter so the bars grow in front of the reader", async () => {
    const main = await openStep();
    const model = within(main).getByRole("figure", { name: "Index memory at basis size B" });
    const play = within(model).getByRole("button", { name: /play through the sizes/i });
    await userEvent.setup().click(play);
    const slider = within(model).getByRole("slider", { name: "Strings in the basis (B)" });
    expect(slider).toHaveAttribute("aria-valuetext", "2^15 = 32,768");
    expect(within(model).getByRole("button", { name: /pause/i })).toHaveAttribute("aria-pressed", "true");
  });

  it("steps through an algorithm and flashes the values that change", async () => {
    const main = await openStep();
    const stepper = within(main).getByRole("figure", { name: "One lookup, step by step" });
    expect(stepper).toHaveTextContent("Step 1 of 4 · line 37");
    expect(within(stepper).getByRole("button", { name: "← Back" })).toBeDisabled();
    const user = userEvent.setup();
    await user.click(within(stepper).getByRole("button", { name: "Next →" }));
    await vi.waitFor(() => expect(stepper).toHaveTextContent("Step 2 of 4 · lines 38-39"));
    expect(within(stepper).getByText("41_027")).toHaveClass("stepper-changed");
    expect(within(stepper).getByText("0x9e37_79b9_7f4a_7c15")).not.toHaveClass("stepper-changed");
    await user.click(within(stepper).getByRole("button", { name: "Next →" }));
    await user.click(within(stepper).getByRole("button", { name: "Next →" }));
    await vi.waitFor(() => expect(stepper).toHaveTextContent("Step 4 of 4"));
    expect(within(stepper).getByRole("button", { name: "Next →" })).toBeDisabled();
    expect(within(stepper).getByRole("button", { name: "↻ Play again" })).toBeInTheDocument();
    // Next became disabled under the keyboard: focus moved to Play again.
    await vi.waitFor(() => expect(within(stepper).getByRole("button", { name: "↻ Play again" })).toHaveFocus());
  });

  it("shows work done every step against work done once, finished at rest", async () => {
    await openStep();
    const nav = screen.getByRole("navigation", { name: /review navigator/i });
    await userEvent.setup().click(within(nav).getByRole("button", { name: /step storage and the parity plan/i }));
    await waitForReady();
    const timeline = within(screen.getByRole("main")).getByRole("figure", { name: "Six steps, before and after" });
    expect(timeline).toHaveTextContent(
      "Before: allocate step workspace every step, build parity tables every step",
    );
    expect(timeline).toHaveTextContent("After: workspace from the pool once, parity plan in new once");
    // Without an observer (tests, screenshots) it rests on the finished picture.
    expect(within(timeline).getByRole("button", { name: "↻ Replay" })).toBeInTheDocument();
  });
});
