//! What every browser test needs before it can ask anything.

use std::path::{Path, PathBuf};

use toy_browser::{Browser, Resources, Url};

pub fn fixture(name: &str) -> Url {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    Url::from_file_path(std::fs::canonicalize(&path).expect("fixture exists")).unwrap()
}

pub fn browser() -> Browser {
    Browser::new(Resources::new(), &[] as &[PathBuf]).expect("a browser")
}
