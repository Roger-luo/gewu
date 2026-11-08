import type { HighlightToken } from "../highlight";

export function TokenLine({
  tokens,
  plain,
}: {
  tokens: HighlightToken[] | undefined;
  plain: string;
}) {
  if (!tokens || tokens.length === 0) {
    return <>{plain}</>;
  }
  return tokens.map((token, index) => (
    <span key={index} style={token.color ? { color: token.color } : undefined}>
      {token.text}
    </span>
  ));
}
