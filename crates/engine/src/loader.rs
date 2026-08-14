//! Module resolution for `<script type="module">`.
//!
//! Specifiers resolve to URLs and the bytes come from [`Resources`]. Bare
//! specifiers are only resolvable through the document's import map, matching
//! what a browser does.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rquickjs::{
    Ctx, Exception, Module, Result,
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
};
use toy_browser_fetch::{Resources, Url};

/// Bare specifier -> mapped specifier, filled from the document's import maps.
/// Shared because the map is only known once the document has been scanned.
pub type ImportMap = Rc<RefCell<HashMap<String, String>>>;

/// Resolves specifiers against the importing module, or against the document
/// for anything the import map rewrote.
pub struct DocumentResolver {
    base_url: Url,
    imports: ImportMap,
}

impl DocumentResolver {
    pub fn new(base_url: Url, imports: ImportMap) -> Self {
        Self { base_url, imports }
    }
}

impl Resolver for DocumentResolver {
    fn resolve<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        base: &str,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        // An import map entry is relative to the document, not to whoever
        // imported the bare name.
        let (specifier, from) = match self.imports.borrow().get(name) {
            Some(mapped) => (mapped.clone(), self.base_url.clone()),
            None => (
                name.to_owned(),
                Url::parse(base).unwrap_or_else(|_| self.base_url.clone()),
            ),
        };

        if !specifier.starts_with('.') && !specifier.starts_with('/') {
            return Err(Exception::throw_message(
                ctx,
                &format!("bare specifier \"{name}\" is not in the import map"),
            ));
        }

        from.join(&specifier)
            .map(|url| url.to_string())
            .map_err(|error| {
                Exception::throw_message(ctx, &format!("cannot resolve \"{name}\": {error}"))
            })
    }
}

/// Reads modules through the shared cache.
pub struct ResourceLoader {
    resources: Resources,
}

impl ResourceLoader {
    pub fn new(resources: Resources) -> Self {
        Self { resources }
    }
}

impl Loader for ResourceLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js, Declared>> {
        let url = Url::parse(name)
            .map_err(|error| Exception::throw_message(ctx, &format!("bad module url: {error}")))?;
        let resource = self.resources.get(&url).map_err(|error| {
            Exception::throw_message(ctx, &format!("cannot load module {name}: {error}"))
        })?;

        Module::declare(ctx.clone(), name, resource.text().into_owned())
    }
}
