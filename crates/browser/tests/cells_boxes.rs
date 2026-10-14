//! Boxes the cell styler maps beyond plain blocks: auto margins, percentage
//! spacing, `calc()`, tables, grid and inline-blocks. Real pages, real CSS.

use toy_browser::{Browser, Resources, Url};

fn text(name: &str, html: &str, width: u16) -> String {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("boxes-{name}.html"));
    std::fs::write(&path, html).expect("a place to write");
    let mut browser = Browser::new(Resources::new()).expect("a browser");
    let tab = browser.new_page().expect("a page");
    let url = Url::from_file_path(&path).expect("a file url");
    browser.navigate(&tab, url.as_str()).expect("loads");
    browser.cells(&tab, width).expect("renders").to_text()
}

// Pins: the centred container, `margin: 0 auto` with a width.
#[test]
fn margin_auto_centres_a_container() {
    let html = "<style>body{margin:0} .c{width:10em;margin:0 auto}</style><div class=c>mid</div>";
    // 10em is 20 columns; a 60 column page leaves 20 on each side.
    assert_eq!(text("auto", html, 60), format!("{}mid", " ".repeat(20)));
}

// Pins: auto margins in a flex row (the nav bar with its link pushed right).
#[test]
fn margin_left_auto_in_flex() {
    let html = "<style>body{margin:0} nav{display:flex} .r{margin-left:auto}</style>
        <nav><a>home</a><a class=r>login</a></nav>";
    assert_eq!(text("navauto", html, 20), "home           login");
}

// Pins: percentage padding is a share of the container, not nothing.
#[test]
fn percentage_padding() {
    let html = "<style>body{margin:0} div{padding:0 10%}</style><div>hi</div>";
    assert_eq!(text("pct", html, 40), "    hi");
}

// Pins: `calc(100% - 4rem)` at the top level resolves against the page width
// (8 columns off an 80 column page), so the border box is 72 wide.
#[test]
fn calc_width_resolves_against_the_page() {
    let html = "<style>body{margin:0} div{width:calc(100% - 4rem);border-right:1px solid}</style>
        <div>x</div>";
    let out = text("calc", html, 80);
    assert_eq!(out.chars().count(), 72, "{out:?}");
}

const TABLE_CSS: &str = "<style>body{margin:0} td,th{border:1px solid}</style>";

// Pins: columns size to their content, and a cell too long for its column
// wraps inside it; the table is as wide as it needs, not the page.
#[test]
fn three_columns_of_uneven_text() {
    let t = format!(
        "{TABLE_CSS}<table><tr><td>short</td><td>a much longer cell of text that must wrap somewhere</td><td>mid cell</td></tr>
        <tr><td>b</td><td>c</td><td>d</td></tr></table>"
    );
    let expected = "\
┌─────┐┌───────────────────────────────┐┌────────┐
│short││a much longer cell of text that││mid cell│
│     ││must wrap somewhere            ││        │
└─────┘└───────────────────────────────┘└────────┘
┌─────┐┌───────────────────────────────┐┌────────┐
│b    ││c                              ││d       │
└─────┘└───────────────────────────────┘└────────┘";
    assert_eq!(text("t3", &t, 50), expected);
}

// Pins: `colspan` spreads a cell over the columns below it.
#[test]
fn colspan_covers_two_columns() {
    let t = format!(
        "{TABLE_CSS}<table><tr><th colspan=2>Title</th></tr><tr><td>left</td><td>right</td></tr></table>"
    );
    let expected = "\
┌───────────┐
│   Title   │
└───────────┘
┌────┐┌─────┐
│left││right│
└────┘└─────┘";
    assert_eq!(text("span", &t, 40), expected);
}

// Pins: `rowspan` holds a column for the rows below, so the next row's first
// cell lands in the second column.
#[test]
fn rowspan_pushes_the_next_row_over() {
    let t = "<style>body{margin:0}</style><table><tr><td rowspan=2>tall</td><td>a</td></tr><tr><td>b</td></tr></table>";
    assert_eq!(text("rowspan", t, 20), "tall a\n     b");
}

// Pins: the shape of a Wikipedia infobox: a caption over th/td rows, the
// header row spanning both columns, row groups looked through, all of it
// inside the article's flow.
#[test]
fn infobox_with_header_rows() {
    let t = format!(
        "{TABLE_CSS}<p>Intro.</p><table><caption>Neovim</caption><tbody>
        <tr><th colspan=2>General</th></tr>
        <tr><th>Written in</th><td>C, Lua</td></tr>
        <tr><th>License</th><td>Apache 2.0</td></tr></tbody></table>"
    );
    let out = text("infobox", &t, 40);
    let expected = "\
Intro.

                 Neovim
┌──────────────────────┐
│       General        │
└──────────────────────┘
┌──────────┐┌──────────┐
│Written in││C, Lua    │
└──────────┘└──────────┘
┌──────────┐┌──────────┐
│ License  ││Apache 2.0│
└──────────┘└──────────┘";
    assert_eq!(out.trim_start(), expected);
}

// Pins: a layout table (no borders) with a picture that could not be fetched:
// the cells sit side by side and the image is its alt text.
#[test]
fn layout_table_with_an_image_placeholder() {
    let t = "<style>body{margin:0}</style><table><tr><td><img src='missing.png' alt='logo'></td>
        <td>Site name<br>tagline</td></tr></table>";
    assert_eq!(text("layout", t, 40), "[logo] Site name\n       tagline");
}

// Pins: `display: grid` with fr tracks and a gap, items filling a row at a time.
#[test]
fn css_grid_with_fr_tracks() {
    let t = "<style>body{margin:0} .g{display:grid;grid-template-columns:1fr 2fr;gap:1em}</style>
        <div class=g><div>one</div><div>two</div><div>three</div></div>";
    assert_eq!(text("grid", t, 40), "one            two\n\nthree");
}

// Pins: inline-blocks are units in a row, a column apart, wrapping whole. The
// border is the first column of the 1em padding's two, so one column inside.
#[test]
fn inline_block_button_row() {
    let t = "<style>body{margin:0} b{display:inline-block;border:1px solid;padding:0 1em}</style>
        <p><b>One</b> <b>Two</b> <b>Three</b></p>";
    let expected = "\
┌─────┐ ┌─────┐ ┌───────┐
│ One │ │ Two │ │ Three │
└─────┘ └─────┘ └───────┘
";
    assert_eq!(text("ib", t, 40).trim_start_matches('\n'), expected);
    let narrow = text("ibnarrow", t, 24);
    assert!(
        narrow.contains("│ Three │") && narrow.lines().count() > 5,
        "{narrow}"
    );
}

// Pins: a picture is sized without a layout: the page's `width` attribute for
// one side and the picture's own shape for the other (a 128 by 128 picture at
// width 64 is 8 columns and 4 rows), and its own size where the page says none.
#[test]
fn pictures_are_sized_from_attributes_and_the_decoded_shape() {
    let png =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/noise.png");
    let shown = |attrs: &str| {
        let html = format!(
            "<style>body{{margin:0}}</style><img src='file://{}' {attrs}>",
            png.display()
        );
        let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("boxes-pic.html");
        std::fs::write(&path, html).expect("a place to write");
        let mut browser = Browser::new(Resources::new()).expect("a browser");
        let tab = browser.new_page().expect("a page");
        let url = Url::from_file_path(&path).expect("a file url");
        browser.navigate(&tab, url.as_str()).expect("loads");
        let grid = browser.cells(&tab, 40).expect("renders");
        grid.pictures().first().map(|p| (p.rect.w, p.rect.h))
    };
    assert_eq!(shown(""), Some((16, 8)));
    assert_eq!(shown("width=64"), Some((8, 4)));
    assert_eq!(shown("height=32"), Some((4, 2)));
    assert_eq!(
        shown("width=640"),
        Some((40, 20)),
        "capped to the page, shape kept"
    );
}

// Pins: styling waits for a linked stylesheet instead of using half a cascade:
// the rule in the file arrives before the tree is read.
#[test]
fn a_linked_stylesheet_is_applied_before_the_tree_is_read() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(
        dir.join("boxes-link.css"),
        "p { display: none } b { font-weight: bold }",
    )
    .expect("a place to write");
    let html = "<link rel=stylesheet href='boxes-link.css'><p>hidden</p><b>shown</b>";
    assert_eq!(text("link", html, 20).trim(), "shown");
}

// Pins: a `<link media>` that does not match this screen is not applied (an
// IE-only or print sheet must not beat the real one), and one that does is.
#[test]
fn a_linked_stylesheet_for_another_medium_is_not_applied() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(dir.join("boxes-media.css"), "p { display: none }").expect("a place to write");
    let link = |media: &str| {
        format!("<link rel=stylesheet href='boxes-media.css' media='{media}'><p>shown</p>")
    };
    assert_eq!(text("m1", &link("print"), 20).trim(), "shown");
    assert_eq!(
        text("m2", &link("all and (-ms-high-contrast: none)"), 20).trim(),
        "shown"
    );
    assert_eq!(text("m3", &link("(min-width: 100px)"), 20).trim(), "");
    assert_eq!(text("m4", &link("screen"), 20).trim(), "");
}

// Pins: CSS `order` reorders flex and grid items (stable, DOM order on ties),
// and does nothing in a block.
#[test]
fn order_reorders_flex_and_grid_items() {
    let item = |order: i32, word: &str| format!("<p style='order:{order};margin:0'>{word}</p>");
    let items = format!("{}{}{}", item(2, "one"), item(1, "two"), item(1, "tre"));
    let flex =
        format!("<style>body{{margin:0}}</style><div style='display:flex;gap:1em'>{items}</div>");
    assert_eq!(text("o1", &flex, 30), "two  tre  one");
    let grid = format!("<style>body{{margin:0}}</style><div style='display:grid'>{items}</div>");
    assert_eq!(text("o2", &grid, 30), "two\ntre\none");
    let block = format!("<style>body{{margin:0}}</style><div>{items}</div>");
    assert_eq!(text("o3", &block, 30), "one\ntwo\ntre");
}

// Pins: `display: contents` makes no box and its children join the parent's
// (here a flex row), instead of vanishing.
#[test]
fn display_contents_hands_its_children_to_the_parent() {
    let html = "<style>body{margin:0} .c{display:contents}</style>\
        <p>a</p><div class=c><p>b</p></div><div style='display:flex;gap:1em'>\
        <div class=c><p style='margin:0'>c</p><p style='margin:0'>d</p></div><p style='margin:0'>e</p></div>";
    let shown = text("contents", html, 30);
    assert!(
        shown.contains("a\n\nb") && shown.contains("c  d  e"),
        "{shown:?}"
    );
}

// Pins: the screen-reader-only idiom draws nothing, and a name given to an
// icon-only link stays on one line.
#[test]
fn visually_hidden_text_is_skipped_and_icon_names_stay_whole() {
    let hidden = "<style>body{margin:0} .sr{position:absolute;width:1px;height:1px;overflow:hidden;clip:rect(1px,1px,1px,1px)}</style>\
        <p>seen<span class=sr>unseen</span></p>";
    let shown = text("sr", hidden, 30);
    assert_eq!(shown.trim(), "seen");
    let icon = "<style>body{margin:0}a{display:block;width:16px}</style><a href=x aria-label='Follow us now'><svg></svg></a>";
    assert_eq!(text("icon", icon, 30).trim(), "Follow us now");
    let over = "<style>body{margin:0}nav{position:absolute;right:0}a{display:block;width:16px}</style><p>Caption</p><nav><a href=x aria-label='Go to the Previous Month'><svg></svg></a></nav>";
    assert!(
        !text("over", over, 30).contains("Previous"),
        "a placed control keeps no name over the page"
    );
}

// Pins: vertical spacing counts in full up to two rows and half beyond them
// (96px is four blank rows above the text, not six), for padding and margin alike.
#[test]
fn large_vertical_spacing_counts_half_past_two_rows() {
    let rows_above = |style: &str| {
        let html = format!("<style>body{{margin:0}}</style><div style='{style}'>x</div>");
        text("spacing", &html, 10)
            .lines()
            .take_while(|row| row.is_empty())
            .count()
    };
    assert_eq!(rows_above("padding:32px 0"), 2, "two rows, as written");
    assert_eq!(rows_above("padding:96px 0"), 4, "four rows, not six");
    assert_eq!(rows_above("margin:96px 0"), 4, "a margin the same");
}

// Pins: buttons in a flex row are flex items (a gap between them, `flex: 1`
// shares the row), not text in one line.
#[test]
fn buttons_in_a_flex_row_are_items() {
    let css = "<style>body{margin:0} .row{display:flex;gap:1em}";
    let plain =
        format!("{css}</style><div class=row><button>Send</button><button>Reset</button></div>");
    assert_eq!(text("b1", &plain, 30), "[Send]  [Reset]");
    let grown = format!(
        "{css} .row button{{flex:1}}</style><div class=row><button>Send</button><button>Reset</button></div>"
    );
    assert_eq!(text("b2", &grown, 30), "    [Send]          [Reset]");
}

// Pins: cells without borders are a column apart (else "Year" and "Release"
// read as one word), and `<table border=1>` draws its lines.
#[test]
fn borderless_cells_are_apart_and_border_attribute_draws() {
    let plain = "<style>body{margin:0}</style><table><tr><td>Year</td><td>Rel</td></tr></table>";
    assert_eq!(text("plain", plain, 20), "Year Rel");
    let ruled =
        "<style>body{margin:0}</style><table border=1><tr><td>a</td><td>b</td></tr></table>";
    assert!(text("ruled", ruled, 20).contains("┌─┐┌─┐"));
}
