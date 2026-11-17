//! Every prompt, with what it is for.

macro_rules! prompts {
    ($($variant:ident => $file:literal, $title:literal, $when:literal;)*) => {
        /// One prompt Gewu sends an agent, named by its file in
        /// `harness/prompts/`.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum Prompt {
            $($variant,)*
        }

        impl Prompt {
            /// Every prompt, in the order a run meets them.
            pub const ALL: &'static [Prompt] = &[$(Prompt::$variant,)*];

            /// The file stem in `harness/prompts/`, e.g. `expand`.
            #[must_use]
            pub fn id(self) -> &'static str {
                match self { $(Prompt::$variant => $file,)* }
            }

            /// A short name for the prompt.
            #[must_use]
            pub fn title(self) -> &'static str {
                match self { $(Prompt::$variant => $title,)* }
            }

            /// When Gewu sends it.
            #[must_use]
            pub fn when(self) -> &'static str {
                match self { $(Prompt::$variant => $when,)* }
            }

            /// The prompt every agent is sent, before any backend's addition.
            #[must_use]
            pub fn general(self) -> &'static str {
                match self {
                    $(Prompt::$variant => include_str!(concat!("../../../harness/prompts/", $file, ".md")),)*
                }
            }

            /// The prompt whose file stem is `id`.
            #[must_use]
            pub fn find(id: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|prompt| prompt.id() == id)
            }
        }
    };
}

prompts! {
    Agent => "agent", "Brief",
        "Once, when a session starts: what Gewu is and how to drive the harness.";
    Continue => "continue", "Continue",
        "When a session that already holds the brief is asked for more.";
    Refused => "refused", "Refused",
        "When Gewu refused a command or an edit the agent asked for.";
    Investigate => "investigate", "Investigate",
        "First: read the change and record what each part does.";
    Research => "research", "Bounded investigation",
        "Instead of Investigate, when a change is too large to read whole.";
    Plan => "plan", "Plan",
        "After investigating: order the change into the work items a reviewer will read.";
    Expand => "expand", "Expand",
        "For each work item: write its chunk of the walkthrough.";
    Rubric => "rubric", "Self-check",
        "Inside Expand and Rewrite: the questions a draft must pass before it is submitted.";
    Proofread => "proofread", "Proofread",
        "After each batch, to a fresh agent: score the chunks against the rubric.";
    Polish => "polish", "Polish",
        "When the proofreader scored chunks below 2: rewrite them to its problems.";
    Awaiting => "awaiting", "Waiting for the reviewer",
        "When a batch is done and the reviewer has not read it yet.";
    Revise => "revise", "Rewrite",
        "When the reviewer asked for changes to the walkthrough.";
    Handover => "handover", "Code changes requested",
        "When the reviewer asked for changes to the code itself.";
    Replan => "replan", "Replan",
        "When the code moved under work that was not yet written.";
    Complete => "complete", "Complete",
        "When the walkthrough is finished and valid.";
}
