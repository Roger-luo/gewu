import type { Contributor } from "./backend/types";

/** The `@name` being typed just before the caret, if any. */
export interface MentionQuery {
  /** Index of the `@`. */
  start: number;
  /** What follows the `@`, possibly empty. */
  query: string;
}

/** GitHub logins: letters, digits, and single hyphens. */
const TYPING = /(^|[^\w@`])@([A-Za-z0-9-]*)$/;

export function mentionQuery(text: string, caret: number): MentionQuery | null {
  const match = TYPING.exec(text.slice(0, caret));
  if (!match) return null;
  const query = match[2];
  return { start: caret - query.length - 1, query };
}

/** Contributors whose login starts with the query, then those containing it. */
export function suggestMentions(
  contributors: Contributor[],
  query: string,
  limit = 6,
): Contributor[] {
  const needle = query.toLowerCase();
  const login = (contributor: Contributor) => contributor.login.toLowerCase();
  const starts = contributors.filter((contributor) => login(contributor).startsWith(needle));
  const contains = contributors.filter(
    (contributor) => !login(contributor).startsWith(needle) && login(contributor).includes(needle),
  );
  return [...starts, ...contains].slice(0, limit);
}

/** Replaces the typed `@query` with `@login ` and puts the caret after it. */
export function completeMention(
  text: string,
  caret: number,
  mention: MentionQuery,
  login: string,
): { text: string; caret: number } {
  const inserted = `@${login} `;
  const after = text.slice(caret).replace(/^[A-Za-z0-9-]*\s?/, "");
  return {
    text: text.slice(0, mention.start) + inserted + after,
    caret: mention.start + inserted.length,
  };
}
