import type { ReactNode } from "react";

/** Line icons for the Settings tabs, drawn like SF Symbols: a 1.6 stroke on a
 *  24 grid, rounded ends, in the current text color. Decorative: each tab
 *  carries its name. */
function Line({ size, children }: { size: number; children: ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable="false"
    >
      {children}
    </svg>
  );
}

/** person.crop.circle: the GitHub account Gewu posts as. */
export function AccountIcon({ size = 24 }: { size?: number }) {
  return (
    <Line size={size}>
      <circle cx="12" cy="12" r="9.25" />
      <circle cx="12" cy="9.5" r="3.25" />
      <path d="M5.9 18.6c1.3-2.3 3.5-3.6 6.1-3.6s4.8 1.3 6.1 3.6" />
    </Line>
  );
}

/** sparkles: the agents that write and proofread. */
export function AgentIcon({ size = 24 }: { size?: number }) {
  return (
    <Line size={size}>
      <path d="M10 3.5l1.6 4.4L16 9.5l-4.4 1.6L10 15.5l-1.6-4.4L4 9.5l4.4-1.6z" />
      <path d="M17.5 13.5l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8z" />
    </Line>
  );
}

/** doc.text: the prompts agents are sent. */
export function PromptIcon({ size = 24 }: { size?: number }) {
  return (
    <Line size={size}>
      <path d="M6.5 3h7.5l4.5 4.5V19a2 2 0 0 1-2 2h-10a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" />
      <path d="M14 3v4.5h4.5" />
      <path d="M8 12h8M8 15h8M8 18h5" />
    </Line>
  );
}

/** internaldrive: where Gewu keeps things. */
export function DriveIcon({ size = 24 }: { size?: number }) {
  return (
    <Line size={size}>
      <rect x="2.75" y="7" width="18.5" height="10" rx="2.5" />
      <path d="M6 14h5" />
      <circle cx="17.5" cy="12" r=".6" fill="currentColor" />
    </Line>
  );
}
