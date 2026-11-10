/** `#rrggbb` colors written as Typst `rgb(...)` or `luma(...)` on one line. */
export function typstColors(line: string): string[] {
  const colors: string[] = [];
  const pattern =
    /rgb\(\s*(?:"#?([0-9a-fA-F]{6})"|([0-9.]+)%?\s*,\s*([0-9.]+)%?\s*,\s*([0-9.]+)%?)\s*\)|luma\(\s*([0-9.]+%?)\s*\)/g;
  for (const match of line.matchAll(pattern)) {
    if (match[1]) {
      colors.push(`#${match[1].toLowerCase()}`);
    } else if (match[2] && match[3] && match[4]) {
      const [red, green, blue] = [match[2], match[3], match[4]].map((part, index) =>
        channel(part, match[0].includes("%") && percentArg(match[0], index)),
      );
      if (red != null && green != null && blue != null) {
        colors.push(hex(red, green, blue));
      }
    } else if (match[5]) {
      const level = channel(match[5].replace("%", ""), match[5].endsWith("%"));
      if (level != null) colors.push(hex(level, level, level));
    }
  }
  return colors;
}

function percentArg(call: string, index: number): boolean {
  const args = call.slice(call.indexOf("(") + 1).split(",");
  return args[index]?.includes("%") ?? false;
}

function channel(text: string, percent: boolean): number | null {
  const number = Number(text);
  if (Number.isNaN(number)) return null;
  const value =
    percent ? (number / 100) * 255
    : text.includes(".") && number >= 0 && number <= 1 ? number * 255
    : number;
  return Math.round(Math.min(255, Math.max(0, value)));
}

function hex(red: number, green: number, blue: number): string {
  return `#${[red, green, blue].map((part) => part.toString(16).padStart(2, "0")).join("")}`;
}
