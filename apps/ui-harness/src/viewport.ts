export interface Viewport {
  label: string;
  width: number;
  height: number;
}

export function parseViewports(spec: string): Viewport[] {
  return spec.split(",").map((part) => {
    const trimmed = part.trim();
    const m = /^(\d+)x(\d+)$/i.exec(trimmed);
    if (!m) throw new Error(`Invalid viewport "${trimmed}"; expected WxH`);
    return { label: trimmed, width: Number(m[1]), height: Number(m[2]) };
  });
}

export function viewportDirName(vp: Viewport, scheme: "light" | "dark"): string {
  return scheme === "dark" ? `${vp.label}-dark` : vp.label;
}
