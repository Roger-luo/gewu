import { typstColors } from "../colorLiteral";
import type { HighlightToken } from "../highlight";

export function TokenLine({
  tokens,
  plain,
  language,
}: {
  tokens: HighlightToken[] | undefined;
  plain: string;
  language?: string | null;
}) {
  const swatches = language === "typst" ? typstColors(plain) : [];
  return (
    <>
      {tokens && tokens.length > 0
        ? tokens.map((token, index) => (
            <span key={index} style={token.color ? { color: token.color } : undefined}>
              {token.text}
            </span>
          ))
        : plain}
      {swatches.map((hex, index) => (
        <span
          key={`${hex}-${index}`}
          className="color-swatch"
          style={{ background: hex }}
          role="img"
          aria-label={`Color ${hex}`}
        />
      ))}
    </>
  );
}
