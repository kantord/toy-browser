//! Whole pages in cells at 80 columns: the shapes real sites have (an article
//! with a nav bar and footer, an encyclopedia entry with an infobox, a form),
//! with their own stylesheets, asserting the lines that show the structure
//! held together rather than every cell.

use toy_browser::{Browser, Resources, Url};

/// The page's rows, trailing space trimmed.
struct Page(Vec<String>);

impl Page {
    fn new(name: &str, html: &str) -> Self {
        let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
        let path = dir.join(format!("pages-{name}.html"));
        std::fs::write(&path, html).expect("a place to write");
        let mut browser = Browser::new(Resources::new()).expect("a browser");
        let tab = browser.new_page().expect("a page");
        let url = Url::from_file_path(&path).expect("a file url");
        browser.navigate(&tab, url.as_str()).expect("loads");
        let text = browser.cells(&tab, 80).expect("renders").to_text();
        Self(text.lines().map(|l| l.trim_end().to_owned()).collect())
    }

    /// The first row containing `needle`.
    fn row(&self, needle: &str) -> &str {
        let found = self.0.iter().find(|row| row.contains(needle));
        found.unwrap_or_else(|| panic!("no row has {needle:?} in {:#?}", self.0))
    }

    /// Every needle is somewhere on the page.
    fn shows(&self, needles: &[&str]) {
        needles.iter().for_each(|needle| {
            self.row(needle);
        });
    }
}

const BLOG: &str = "<!doctype html><html><head><title>Blog</title><style>
body { margin: 0; font-family: sans-serif }
header { background: #222; color: #fff; padding: 0 1em }
nav { display: flex; gap: 2em; max-width: 60em; margin: 0 auto }
nav a { color: #fff; text-decoration: none }
nav .right { margin-left: auto }
main { max-width: 40em; margin: 1em auto; padding: 0 1em }
h1 { margin: 0.5em 0 }
.meta { color: #666 }
blockquote { margin: 1em 2em; border-left: 3px solid #999; padding-left: 1em }
footer { border-top: 1px solid #ccc; text-align: center; padding: 1em 0 }
footer span { display: inline-block; padding: 0 1em }
</style></head><body>
<header><nav><a href='/'>Home</a><a href='/posts'>Posts</a><a class=right href='/login'>Log in</a></nav></header>
<main><article>
<h1>Cells all the way down</h1>
<p class=meta>Posted by <b>Ada</b> on 2026-10-12</p>
<p>A terminal is a grid of cells, and a browser that knows it lays out in cells
instead of pixels. This paragraph is long enough to wrap onto a second line at
the width of this article, which is forty em.</p>
<blockquote>Round the edges, never the sizes.</blockquote>
<ul><li>Taffy in whole cells</li><li>Text as a measure function</li></ul>
</article></main>
<footer><span>About</span> <span>Contact</span> <span>RSS</span></footer>
</body></html>";

// Pins: the nav is a flex row with its last link pushed right by an auto
// margin; the article is a centred column (`margin: 0 auto`, a max-width in
// em); the footer's inline-blocks sit in one centred row under a rule.
#[test]
fn blog_nav_pushes_log_in_to_the_right() {
    let page = Page::new("blog", BLOG);
    let nav = page.row("Home").trim_start();
    assert!(nav.starts_with("Home    Posts"), "{nav:?}");
    assert!(nav.ends_with("Log in"), "{nav:?}");
    assert_eq!(nav.chars().count(), 76, "{nav:?}");
}

#[test]
fn blog_article_is_a_centred_column() {
    let page = Page::new("blog", BLOG);
    // The column is 40em = 80 columns, so on this page it is the whole width
    // less its 1em padding: two columns in.
    assert!(page.row("Cells all the way down").starts_with("  Cells"));
    assert!(page.row("Posted by").ends_with("Ada on 2026-10-12"));
    page.shows(&["│ Round the edges", "• Taffy in whole cells", "────"]);
}

#[test]
fn blog_footer_is_a_centred_row() {
    let footer = Page::new("blog", BLOG).row("About").to_owned();
    assert!(footer.starts_with("          "), "centred: {footer:?}");
    assert!(
        footer.ends_with("RSS") && footer.contains("Contact"),
        "{footer:?}"
    );
}

const WIKI: &str = "<!doctype html><html><head><title>Neovim</title><style>
body { margin: 0 }
#content { max-width: 70em; margin: 0 auto; padding: 0 1em }
.infobox { float: right; width: 22em; border: 1px solid #aaa; background: #f8f9fa; margin: 0 0 1em 1em }
.infobox th, .infobox td { padding: 0 0.5em; vertical-align: top }
.infobox caption { font-weight: bold }
.toc { border: 1px solid #aaa; display: inline-block; padding: 0 1em }
</style></head><body><div id=content>
<h1>Neovim</h1>
<table class=infobox><caption>Neovim</caption>
<tr><td colspan=2 style='text-align:center'><img src='missing.png' alt='Neovim logo'></td></tr>
<tr><th>Developer</th><td>Neovim community</td></tr>
<tr><th>Written in</th><td>C, Lua, Vim script</td></tr>
<tr><th>License</th><td>Apache-2.0</td></tr>
</table>
<p><b>Neovim</b> is a refactor of the <a href='/vim'>Vim</a> text editor, aimed at extensibility and usability.</p>
<div class=toc><b>Contents</b><ol><li>History</li><li>Features</li></ol></div>
<h2>Features</h2>
<ul><li>Built-in terminal</li><li>Asynchronous job control</li><li>Lua scripting</li></ul>
<ol><li>First</li><li>Second</li></ol>
<table border=1><tr><th>Year</th><th>Release</th></tr><tr><td>2015</td><td>0.1</td></tr><tr><td>2024</td><td>0.10</td></tr></table>
</div></body></html>";

// Pins: the infobox is a bordered table with its caption and a spanning
// logo row (here its alt text); the lead paragraph, the contents box (an
// inline-block), bullets and numbers all come through, and the data table has
// its header cells.
#[test]
fn encyclopedia_entry() {
    let page = Page::new("wiki", WIKI);
    page.shows(&[
        "[Neovim logo]",
        "Developer",
        "Neovim community",
        "Written in",
    ]);
    page.shows(&[
        "C, Lua, Vim script",
        "Apache-2.0",
        "Neovim is a refactor of the Vim text editor",
    ]);
    page.shows(&[
        "• Built-in terminal",
        "• Lua scripting",
        "1. First",
        "2. Second",
    ]);
    page.shows(&["1. History", "Contents"]);
    assert!(page.row("Year").contains("Release"));
    assert!(page.row("0.10").contains("2024"));
    // `<table border=1>` draws a box round the table and each cell.
    page.shows(&["┌────┐┌───────┐"]);
}

const FORM: &str = "<!doctype html><html><head><style>
form { max-width: 30em; margin: 1em auto }
label { display: block; margin: 0.5em 0 }
.row { display: flex; gap: 1em }
.row button { flex: 1 }
fieldset { border: 1px solid #888; padding: 0.5em 1em }
</style></head><body><form action='/go'>
<fieldset><legend>Sign up</legend>
<label>Name <input type=text name=n value='Ada'></label>
<label>Email <input type=email name=e placeholder='you@example.com'></label>
<label><input type=checkbox name=t checked> I agree to the terms</label>
<label>Plan <select name=p><option>Free</option><option selected>Pro</option></select></label>
<label>Notes <textarea name=x>hello</textarea></label>
</fieldset>
<div class=row><button type=submit>Send</button><button type=reset>Reset</button></div>
</form></body></html>";

// Pins: controls read as their text form, labels stack as blocks, the
// fieldset is boxed, and two buttons in a flex row share the row.
#[test]
fn form_page() {
    let page = Page::new("form", FORM);
    page.shows(&[
        "Name [Ada",
        "Email [",
        "[x] I agree to the terms",
        "Plan [Pro",
        "Notes [hello",
    ]);
    page.shows(&["┌", "└", "Sign up"]);
    let buttons = page.row("[Send]");
    assert!(buttons.contains("[Reset]"), "{buttons:?}");
    // `flex: 1` makes each take half the row, so Reset is far to the right.
    assert!(buttons.chars().count() > 40, "{buttons:?}");
}
