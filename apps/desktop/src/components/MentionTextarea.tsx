import {
  forwardRef,
  useId,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";

import type { Contributor } from "../backend/types";
import {
  completeMention,
  mentionQuery,
  suggestMentions,
  type MentionQuery,
} from "../mentions";

/**
 * A textarea that suggests the repository's contributors after `@`.
 * Arrow keys move through the list, Enter or Tab picks, Escape closes it.
 */
export const MentionTextarea = forwardRef<
  HTMLTextAreaElement,
  {
    value: string;
    onChange: (value: string) => void;
    contributors: Contributor[];
  }
>(function MentionTextarea({ value, onChange, contributors }, ref) {
  const listId = useId();
  const textarea = useRef<HTMLTextAreaElement>(null);
  useImperativeHandle(ref, () => textarea.current as HTMLTextAreaElement);
  /** Where the caret goes once a picked mention has rendered. */
  const pendingCaret = useRef<number | null>(null);
  const [mention, setMention] = useState<MentionQuery | null>(null);
  const [active, setActive] = useState(0);
  const [caret, setCaret] = useState(0);
  const matches = mention ? suggestMentions(contributors, mention.query) : [];
  const open = matches.length > 0;

  const track = (text: string, at: number) => {
    const next = mentionQuery(text, at);
    setCaret(at);
    setMention(next);
    if (next?.query !== mention?.query) setActive(0);
  };

  useLayoutEffect(() => {
    if (pendingCaret.current === null) return;
    textarea.current?.setSelectionRange(pendingCaret.current, pendingCaret.current);
    pendingCaret.current = null;
  }, [value]);

  const pick = (login: string) => {
    if (!mention) return;
    const done = completeMention(value, caret, mention, login);
    pendingCaret.current = done.caret;
    setCaret(done.caret);
    setMention(null);
    onChange(done.text);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (!open) return;
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key];
    if (step) {
      event.preventDefault();
      setActive((current) => (current + step + matches.length) % matches.length);
    } else if (event.key === "Enter" || event.key === "Tab") {
      event.preventDefault();
      pick(matches[active].login);
    } else if (event.key === "Escape") {
      // Closes the list, not the dialog.
      event.preventDefault();
      event.stopPropagation();
      setMention(null);
    }
  };

  return (
    <div className="mention-field">
      <textarea
        ref={textarea}
        value={value}
        aria-autocomplete="list"
        aria-controls={open ? listId : undefined}
        aria-activedescendant={open ? `${listId}-${active}` : undefined}
        onChange={(event) => {
          onChange(event.target.value);
          track(event.target.value, event.target.selectionStart);
        }}
        onSelect={(event) =>
          track(event.currentTarget.value, event.currentTarget.selectionStart)
        }
        onBlur={() => setMention(null)}
        onKeyDown={onKeyDown}
      />
      {open && (
        <ul id={listId} className="mention-list" role="listbox" aria-label="Mention a contributor">
          {matches.map((contributor, index) => (
            <li
              key={contributor.login}
              id={`${listId}-${index}`}
              role="option"
              aria-selected={index === active}
              className={index === active ? "active" : undefined}
              // Keeps the textarea focused so the pick lands at its caret.
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => pick(contributor.login)}
            >
              <img src={contributor.avatar_url} alt="" width={18} height={18} />
              <span>@{contributor.login}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
});
