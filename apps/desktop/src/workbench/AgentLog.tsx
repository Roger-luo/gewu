import { useEffect, useRef, useState } from "react";

import type { WorkbenchBackend } from "./backend";
import type { LogEvent, LogKind, TaskView } from "./protocol";
import { InlineMarkdown } from "../components/Markdown";
import { readableLogText } from "./logText";

const KIND_LABELS: Record<LogKind, string> = {
  output: "Agent",
  tool: "Tool",
  status: "Gewu",
  denied: "Refused",
  error: "Error",
};

/** The task's events, appended as they arrive. Key it by task id. */
export function AgentLog({ backend, task }: { backend: WorkbenchBackend; task: TaskView }) {
  const [events, setEvents] = useState<LogEvent[]>([]);
  const [failure, setFailure] = useState<string | null>(null);
  const next = useRef(0);
  const list = useRef<HTMLDivElement>(null);
  const running = task.status === "generating";

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const log = await backend.log(task.id, next.current);
        if (cancelled) {
          return;
        }
        next.current = log.next;
        setFailure(null);
        if (log.events.length > 0) {
          setEvents((current) => [...current, ...log.events.filter((event) => event.seq >= current.length)]);
        }
      } catch (error) {
        if (!cancelled) {
          setFailure(error instanceof Error ? error.message : String(error));
        }
      }
    };
    void poll();
    const timer = running ? setInterval(() => void poll(), 800) : undefined;
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [backend, running, task.id, task.updated_at]);

  // Tool calls (reads, edits, commands) are how the agent worked, not what it
  // found; they stay folded until asked for. Found by the final UI judge.
  const [showTools, setShowTools] = useState(false);
  const tools = events.filter((event) => event.kind === "tool").length;
  const shown = showTools ? events : events.filter((event) => event.kind !== "tool");

  useEffect(() => {
    const element = list.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [shown.length]);

  return (
    <section className="panel agent-log" aria-labelledby="agent-log-title">
      <div className="agent-log-head">
        <h3 id="agent-log-title">Agent log</h3>
        {tools > 0 && (
          <button
            type="button"
            className="btn small"
            aria-pressed={showTools}
            onClick={() => setShowTools(!showTools)}
          >
            {showTools ? "Hide tool calls" : `Show tool calls (${tools})`}
          </button>
        )}
      </div>
      {failure && <p className="warning">Could not read the log: {failure}</p>}
      {events.length === 0 ? (
        <p className="muted">{running ? "Waiting for the first event…" : "Nothing was logged."}</p>
      ) : (
        <div ref={list} className="log-lines" role="log" aria-label="Agent log" tabIndex={0}>
          <ol>
            {shown.map((event) => (
              <li key={event.seq} className={`log-line ${event.kind}`}>
                <time dateTime={new Date(event.at).toISOString()}>
                  {new Date(event.at).toLocaleTimeString(undefined, {
                    hour: "2-digit",
                    minute: "2-digit",
                    second: "2-digit",
                  })}
                </time>
                <span className="log-kind">{KIND_LABELS[event.kind]}</span>
                <span className="log-text">
                  <InlineMarkdown source={readableLogText(event.text)} />
                </span>
              </li>
            ))}
          </ol>
        </div>
      )}
    </section>
  );
}
