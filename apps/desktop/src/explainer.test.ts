import { compileFormula, formatValue, paramSteps } from "./explainer";

const evaluate = (text: string, names: string[] = [], values: number[] = []) =>
  compileFormula(text, names)(values);

describe("explainer formulas", () => {
  it("match the Rust grammar's precedence", () => {
    expect(evaluate("1 + 2 * 3")).toBe(7);
    expect(evaluate("(1 + 2) * 3")).toBe(9);
    expect(evaluate("2^3^2")).toBe(512);
    expect(evaluate("-2^2")).toBe(-4);
    expect(evaluate("2^-1")).toBe(0.5);
    expect(evaluate("n * 41", ["n"], [1024])).toBe(41_984);
    expect(evaluate("next_pow2(n * 8 / 7)", ["n"], [524_288])).toBe(1_048_576);
    expect(evaluate("max(log2(n), ceil(1.5))", ["n"], [8])).toBe(3);
  });

  it("refuse undeclared names and unknown functions", () => {
    expect(() => compileFormula("n * m", ["n"])).toThrow("`m` is not a declared parameter");
    expect(() => compileFormula("exec(n)", ["n"])).toThrow("unknown function `exec`");
    expect(() => compileFormula("min(n)", ["n"])).toThrow("takes 2 argument(s), got 1");
    expect(() => compileFormula("n $ 2", ["n"])).toThrow("unexpected `$`");
  });

  it("format values and offer power-of-two steps", () => {
    expect(formatValue(41 * 1_048_576, "bytes")).toBe("41.0 MiB");
    expect(formatValue(1_234_567)).toBe("1,234,567");
    expect(paramSteps({ id: "b", label: "B", min: 100, max: 1024, default: 512, scale: "log2" })).toEqual([
      128, 256, 512, 1024,
    ]);
  });
});
