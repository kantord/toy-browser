//! Reading bytes over HTTP.
//!
//! One agent for the process, because connection reuse is most of what makes a
//! second request to the same host cheap. Blocking, like everything else here:
//! the cache is what callers rely on for speed, not concurrency.

use std::{sync::OnceLock, time::Duration};

use ureq::ResponseExt as _;

use crate::{FetchError, Url};

/// How long a single request may take before the render gives up on it. A page
/// that hangs should fail rather than stop the browser answering.
const TIMEOUT: Duration = Duration::from_secs(15);

/// What this browser calls itself. Honest rather than an imitation: a server
/// that decides to serve something else is telling us something worth seeing.
const USER_AGENT: &str = concat!("toy-browser/", env!("CARGO_PKG_VERSION"));

/// What came back, and where from — which is not always where we asked, because
/// a redirect moves the base every relative reference resolves against.
pub(crate) struct Fetched {
    pub(crate) url: Url,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn get(url: &Url) -> Result<Fetched, FetchError> {
    let mut response = agent()
        .get(url.as_str())
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(|error| transport_error(url, &error))?;

    let landed = Url::parse(response.get_uri().to_string().as_str()).unwrap_or_else(|_| url.clone());
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()
        .map_err(|error| FetchError::Unreadable {
            url: url.clone(),
            reason: error.to_string(),
        })?;

    Ok(Fetched { url: landed, bytes })
}

/// A ceiling on one response, so a stream that never ends cannot take the
/// process with it. Generous next to any document worth rendering.
const MAX_BODY: u64 = 32 * 1024 * 1024;

/// A status the server chose is a different thing from a connection that never
/// worked, and a caller wanting to report either needs to tell them apart.
fn transport_error(url: &Url, error: &ureq::Error) -> FetchError {
    match error {
        ureq::Error::StatusCode(404 | 410) => FetchError::NotFound(url.clone()),
        ureq::Error::StatusCode(status) => FetchError::Unreadable {
            url: url.clone(),
            reason: format!("server said {status}"),
        },
        other => FetchError::Unreadable {
            url: url.clone(),
            reason: other.to_string(),
        },
    }
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .build()
            .into()
    })
}
