//! Declaring a class's JavaScript surface as a table rather than a wall.
//!
//! A class's members all have to be written into one `#[rquickjs::methods]`
//! block — the macro that builds the prototype only gets one look at the impl —
//! so the surface cannot be split across files the way ordinary code is. Most
//! of it is also the same four shapes repeated: read an attribute as text, read
//! one as a flag, answer with a wrapper, answer with a list of them.
//!
//! So the repetition is declared once here and the members are listed, which
//! keeps the surface readable at a glance and leaves the file arguing about
//! behaviour rather than about boilerplate.

/// Builds the `#[rquickjs::methods]` block for a node class.
///
/// Each group takes `rust_name "jsName" => …`; the Rust name is separate
/// because some JavaScript members (`type`, `id`) are not usable as Rust
/// function names. `rest { … }` is written out longhand for anything that does
/// not fit a shape.
macro_rules! dom_members {
    (
        $class:ident;
        $(text { $($tr:ident $tj:literal => $ta:literal),* $(,)? })?
        $(text_rw { $($wr:ident / $ws:ident $wj:literal => $wa:literal),* $(,)? })?
        $(flag { $($fr:ident $fj:literal => $fa:literal),* $(,)? })?
        $(node { $($nr:ident $nj:literal => |$ns:ident| $nb:expr),* $(,)? })?
        $(list { $($lr:ident $lj:literal => |$ls:ident| $lb:expr),* $(,)? })?
        $(object { $($or:ident $oj:literal => |$oc:ident, $os:ident| $ob:expr),* $(,)? })?
        rest { $($rest:tt)* }
    ) => {
        #[rquickjs::methods]
        impl $class {
            $($(
                /// An attribute read as text. Absent reads as empty, as the DOM says.
                #[qjs(get, rename = $tj)]
                pub fn $tr(&self) -> String {
                    self.dom.attribute(self.id, $ta).unwrap_or_default()
                }
            )*)?

            $($(
                /// An attribute read and written as text.
                #[qjs(get, rename = $wj)]
                pub fn $wr(&self) -> String {
                    self.dom.attribute(self.id, $wa).unwrap_or_default()
                }

                #[qjs(set, rename = $wj)]
                pub fn $ws(&self, value: Coerced<String>) {
                    self.dom.set_attribute(self.id, $wa, &value.0);
                }
            )*)?

            $($(
                /// An attribute read as a flag: present is true whatever it says.
                #[qjs(get, rename = $fj)]
                pub fn $fr(&self) -> bool {
                    self.dom.attribute(self.id, $fa).is_some()
                }
            )*)?

            $($(
                #[qjs(get, rename = $nj)]
                pub fn $nr<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Value<'js>> {
                    let $ns = self;
                    wrap_maybe(&ctx, $nb)
                }
            )*)?

            $($(
                #[qjs(get, rename = $lj)]
                pub fn $lr<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<Value<'js>>> {
                    let $ls = self;
                    wrap_all(&ctx, $lb)
                }
            )*)?

            $($(
                #[qjs(get, rename = $oj)]
                pub fn $or<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Object<'js>> {
                    let ($oc, $os) = (ctx, self);
                    $ob
                }
            )*)?

            $($rest)*
        }
    };
}

pub(super) use dom_members;
