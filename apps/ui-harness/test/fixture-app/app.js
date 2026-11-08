(() => {
  const pages = [
    { id: "overview", label: "Overview", target: { screen: "overview" } },
    { id: "chunk-auth", label: "Auth flow", target: { screen: "chunk", index: 0 } },
    { id: "chunk-api", label: "API layer", target: { screen: "chunk", index: 1 } },
    { id: "summary", label: "Summary", target: { screen: "summary" } },
  ];

  const copy = {
    overview:
      "A short fixture walkthrough with three destinations. Use j/k to move, ? for help.",
    "chunk-auth":
      "Review the authentication entry points. Look for session handling and redirect edges.",
    "chunk-api":
      "Inspect the HTTP bridge and how snapshots are served to the web UI.",
    summary: "Two chunks reviewed in this fixture. Decide approve, comment, or request changes.",
  };

  let index = 0;
  let overlay = null;
  let transitioning = false;

  const main = document.getElementById("main");
  const title = document.getElementById("title");
  const body = document.getElementById("body");
  const context = document.getElementById("context");
  const navList = document.getElementById("nav-list");
  const help = document.getElementById("help");
  const feedback = document.getElementById("feedback");
  const status = document.getElementById("status");

  function setReady(ready) {
    document.documentElement.dataset.gewuReady = ready ? "true" : "false";
  }

  function render() {
    const page = pages[index];
    title.textContent = page.label;
    body.textContent = copy[page.id];
    main.dataset.gewuScreen = page.target.screen;
    if (page.target.screen === "chunk") {
      main.dataset.gewuChunkId = page.id;
      context.hidden = false;
    } else {
      delete main.dataset.gewuChunkId;
      context.hidden = true;
    }
    for (const btn of navList.querySelectorAll("button")) {
      btn.setAttribute("aria-current", btn.dataset.id === page.id ? "page" : "false");
    }
    status.textContent = `${page.label} (${index + 1}/${pages.length})`;
  }

  function buildNav() {
    navList.innerHTML = "";
    pages.forEach((p, i) => {
      const li = document.createElement("li");
      const btn = document.createElement("button");
      btn.type = "button";
      btn.textContent = p.label;
      btn.dataset.id = p.id;
      btn.addEventListener("click", () => void goto(p.target));
      li.appendChild(btn);
      navList.appendChild(li);
      if (i === 0) btn.setAttribute("aria-current", "page");
    });
  }

  function state() {
    const page = pages[index];
    return {
      screen: page.target.screen,
      chunk_index: page.target.screen === "chunk" ? page.target.index : null,
      chunk_count: pages.filter((p) => p.target.screen === "chunk").length,
      overlay,
    };
  }

  async function goto(target) {
    transitioning = true;
    setReady(false);
    const next = pages.findIndex((p) => {
      if (target.screen === "chunk") {
        return p.target.screen === "chunk" && p.target.index === target.index;
      }
      return p.target.screen === target.screen;
    });
    if (next >= 0) index = next;
    render();
    await new Promise((r) => setTimeout(r, 20));
    transitioning = false;
    setReady(true);
  }

  function openHelp() {
    overlay = "help";
    help.showModal();
  }

  function openFeedback() {
    overlay = "feedback";
    feedback.showModal();
  }

  function closeOverlays() {
    if (help.open) help.close();
    if (feedback.open) feedback.close();
    overlay = null;
  }

  help.addEventListener("close", () => {
    if (overlay === "help") overlay = null;
  });
  feedback.addEventListener("close", () => {
    if (overlay === "feedback") overlay = null;
  });
  document.getElementById("feedback-cancel").addEventListener("click", () => {
    feedback.close();
  });
  document.getElementById("feedback-form").addEventListener("submit", (e) => {
    e.preventDefault();
    feedback.close();
  });

  document.addEventListener("keydown", (e) => {
    if (transitioning) return;
    const inField =
      e.target instanceof HTMLElement &&
      (e.target.tagName === "INPUT" ||
        e.target.tagName === "TEXTAREA" ||
        e.target.tagName === "SELECT");
    if (inField) return;

    if (e.key === "Escape") {
      closeOverlays();
      return;
    }
    if (overlay) return;

    if (e.key === "?") {
      e.preventDefault();
      openHelp();
      return;
    }
    if (e.key === "j") {
      e.preventDefault();
      if (index < pages.length - 1) void goto(pages[index + 1].target);
      return;
    }
    if (e.key === "k") {
      e.preventDefault();
      if (index > 0) void goto(pages[index - 1].target);
      return;
    }
    if (e.key === "o") {
      e.preventDefault();
      void goto({ screen: "overview" });
      return;
    }
    if (e.key === "s") {
      e.preventDefault();
      void goto({ screen: "summary" });
      return;
    }
    if (e.key === "c") {
      e.preventDefault();
      openFeedback();
    }
  });

  window.__gewu = {
    state,
    goto,
    pages: () => pages.map((p) => ({ ...p, target: { ...p.target } })),
  };

  buildNav();
  render();
  setReady(true);
})();
