//! Module resolution for `<script type="module">`.
//!
//! Specifiers resolve to files on disk. Bare specifiers are only resolvable
//! through the document's import map, matching what a browser does — with the
//! difference that a browser would also accept a URL.

use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Component, Path, PathBuf},
    rc::Rc,
};

use rquickjs::{
    Ctx, Exception, Module, Result,
    loader::{ImportAttributes, Loader, Resolver},
    module::Declared,
};

/// Bare specifier -> mapped specifier, filled from the document's import maps.
/// Shared because the map is only known once the document has been scanned.
pub type ImportMap = Rc<RefCell<HashMap<String, String>>>;

/// Resolves specifiers against the importing module, or against the document
/// for anything the import map rewrote.
pub struct DocumentResolver {
    base_dir: PathBuf,
    imports: ImportMap,
}

impl DocumentResolver {
    pub fn new(base_dir: &Path, imports: ImportMap) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            imports,
        }
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
            Some(mapped) => (mapped.clone(), self.base_dir.clone()),
            None => (
                name.to_owned(),
                Path::new(base)
                    .parent()
                    .map_or_else(|| self.base_dir.clone(), Path::to_path_buf),
            ),
        };

        if !specifier.starts_with('.') && !specifier.starts_with('/') {
            return Err(Exception::throw_message(
                ctx,
                &format!("bare specifier \"{name}\" is not in the import map"),
            ));
        }

        Ok(normalize(&from.join(specifier)).display().to_string())
    }
}

/// Reads modules off the filesystem.
pub struct FileLoader;

impl Loader for FileLoader {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _attributes: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js, Declared>> {
        let source = std::fs::read_to_string(name).map_err(|error| {
            Exception::throw_message(ctx, &format!("cannot load module {name}: {error}"))
        })?;
        Module::declare(ctx.clone(), name, source)
    }
}

/// Collapses `.` and `..` without touching the filesystem, so a module that
/// does not exist still resolves to a stable name for the error message.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}
