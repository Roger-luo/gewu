import { readableLogText } from "./logText";

describe("agent log text", () => {
  it("turns harness headings into plain stage words", () => {
    expect(readableLogText("Stopped: # Gewu harness: waiting for the reviewer")).toBe(
      "Stopped: waiting for the reviewer",
    );
    expect(readableLogText("# COMPLETE")).toBe("complete");
    expect(readableLogText("Ran `gewu harness next`")).toBe("Ran `gewu harness next`");
  });
});
