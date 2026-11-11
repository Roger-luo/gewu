import { useEffect, useState } from "react";

import type { Backend, Contributor } from "../backend/types";

/**
 * Who a note can `@`-mention, read when the comment dialog opens. `login`
 * changes on sign-in and sign-out, which changes who GitHub lists. A failed
 * read leaves the list empty: mentions are a convenience.
 */
export function useContributors(
  backend: Backend,
  active: boolean,
  login: string | null,
): Contributor[] {
  const [contributors, setContributors] = useState<Contributor[]>([]);

  useEffect(() => {
    if (!active || !backend.contributors) return;
    let live = true;
    backend.contributors().then(
      (listed) => live && setContributors(listed),
      () => live && setContributors([]),
    );
    return () => {
      live = false;
    };
  }, [backend, active, login]);

  return login ? contributors : [];
}
