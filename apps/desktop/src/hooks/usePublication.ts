import { useCallback, useEffect, useState } from "react";

import type { Backend } from "../backend/types";
import type { Publication, Receipt } from "../publication";

export interface PublicationState {
  publication: Publication | null;
  error: string | null;
  /** Absent where the host cannot send, e.g. a single review window. */
  publish?: () => Promise<Receipt>;
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/**
 * The draft the summary screen previews. A backend with a publication
 * endpoint builds it; otherwise `local` does. `revision` changes whenever
 * the notes do, so the draft follows them.
 */
export function usePublication(
  backend: Backend,
  local: () => Publication,
  revision: string,
  active: boolean,
): PublicationState {
  const [{ fetched, error }, setState] = useState<{
    fetched: Publication | null;
    error: string | null;
  }>({ fetched: null, error: null });

  const load = useCallback(async () => {
    if (!backend.publication) {
      return;
    }
    try {
      const fetched = await backend.publication();
      setState({ fetched, error: null });
    } catch (err) {
      setState((current) => ({ ...current, error: message(err) }));
    }
  }, [backend]);

  useEffect(() => {
    if (active) {
      void load();
    }
  }, [active, revision, load]);

  const send = backend.publish?.bind(backend);
  const publish = send
    ? async () => {
        const receipt = await send();
        await load();
        return receipt;
      }
    : undefined;

  return {
    publication: backend.publication ? fetched : local(),
    error,
    publish,
  };
}
