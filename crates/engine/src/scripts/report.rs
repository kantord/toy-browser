//! Rendering a survey for a person to read.
//!
//! The labels exist only for this: nothing in the engine branches on them, so
//! changing the wording of a report cannot change what the survey found.

use super::{EntryKind, Fetch, Payload, ScriptSurvey, Timing};

impl EntryKind {
    fn label(self) -> &'static str {
        match self {
            Self::ClassicScript => "classic script",
            Self::ModuleScript => "module script",
            Self::NoModuleScript => "nomodule script",
            Self::ImportMap => "import map",
            Self::DataBlock => "data block",
            Self::EventHandlerAttribute => "event handler attribute",
            Self::JavascriptUrl => "javascript: URL",
            Self::PreloadHint => "preload hint",
        }
    }
}

impl Timing {
    fn label(self) -> &'static str {
        match self {
            Self::ParserBlocking => "parser-blocking",
            Self::Deferred => "deferred",
            Self::Async => "async",
            Self::ResourceEvent => "resource event",
            Self::UserEvent => "user event",
            Self::NotExecuted => "not executed",
        }
    }
}

impl ScriptSurvey {
    /// A human-readable report of the survey.
    pub fn to_markdown(&self, title: &str) -> String {
        let mut out = format!("# Script entry points: {title}\n\n");

        if self.entry_points.is_empty() {
            out.push_str("No JavaScript entry points found.\n");
            return out;
        }

        for (index, entry) in self.entry_points.iter().enumerate() {
            out.push_str(&format!(
                "## {}. {} ({})\n\n- origin: `{}`\n",
                index + 1,
                entry.kind.label(),
                entry.timing.label(),
                entry.origin
            ));

            match &entry.payload {
                Payload::Inline { source } => {
                    out.push_str(&format!("- inline, {} bytes\n\n", source.len()));
                }
                Payload::External { specifier, fetch } => {
                    out.push_str(&format!("- specifier: `{specifier}`\n"));
                    match fetch {
                        Fetch::Loaded { url, source } => out.push_str(&format!(
                            "- loaded `{url}`, {} bytes\n\n",
                            source.len()
                        )),
                        Fetch::NotFound { url } => {
                            out.push_str(&format!("- NOT FOUND at `{url}`\n\n"));
                        }
                        Fetch::Unsupported { url, reason } => {
                            out.push_str(&format!("- not fetched: {url} ({reason})\n\n"));
                        }
                    }
                }
            }
        }

        out
    }
}
