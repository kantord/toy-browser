//! What a script can ask a document about the network: reading a URL through
//! the page's cache, and taking a URL apart. Only scripts ask, so only a build
//! with an interpreter has it.

use toy_browser_fetch::Url;

use super::Dom;

impl Dom {
    /// How many times this DOM has changed.
    /// Reads a URL through the same cache the document and its scripts came
    /// through, resolved against the page.
    ///
    /// The one thing a page can ask the network for after it has loaded, and it
    /// asks for it here rather than through a connection of its own: a `fetch`
    /// with its own cache would read a file the page already has twice, and
    /// would answer differently from the `<img>` beside it.
    ///
    /// Blocking, because the cache is. Nothing is gained by making the wait
    /// asynchronous when there is no thread for it to happen on — the promise
    /// the page is handed is already settled.
    pub fn read(&self, url: &str) -> Result<(String, String, u16), String> {
        let target = self
            .base_url
            .join(url)
            .map_err(|_| format!("not a url: {url}"))?;
        match self.resources.get(&target) {
            // Whatever the server said, including that it will not say. A
            // status is an *answer*, not a failure to ask: a page reads it as
            // `response.ok === false` and carries on, often on the body that
            // came with it. Turning one into a network error instead rejects a
            // promise nobody expected to reject, and a page that treats that as
            // "the network is gone" stops doing everything else as well.
            Ok(resource) => Ok((
                target.to_string(),
                resource.text().into_owned(),
                resource.status,
            )),
            // Nothing answered at all — no server, no file, no scheme anyone
            // here can read. That is the one case `fetch` rejects on.
            Err(toy_browser_fetch::FetchError::NotFound(_)) => {
                Ok((target.to_string(), String::new(), 404))
            }
            Err(error) => Err(error.to_string()),
        }
    }

    /// A URL taken apart, resolved against `base` or against the page.
    ///
    /// Parsed by the same crate that resolves every other reference the
    /// document makes, rather than by a regular expression in the prelude: a
    /// page whose router disagrees with its own `<a href>` about what a path is
    /// is a page that navigates somewhere it did not mean to.
    ///
    /// Empty when it does not parse, which is what `URL` throws on.
    pub fn parse_url(&self, href: &str, base: Option<String>) -> Vec<String> {
        let against = base
            .and_then(|it| Url::parse(&it).ok())
            .unwrap_or_else(|| self.base_url.clone());
        let Ok(url) = against.join(href) else {
            return Vec::new();
        };
        vec![
            url.to_string(),
            format!("{}:", url.scheme()),
            url.host_str().map(str::to_owned).unwrap_or_default(),
            url.port().map(|it| it.to_string()).unwrap_or_default(),
            url.path().to_owned(),
            url.query().map(|it| format!("?{it}")).unwrap_or_default(),
            url.fragment()
                .map(|it| format!("#{it}"))
                .unwrap_or_default(),
            url.origin().ascii_serialization(),
        ]
    }
}
