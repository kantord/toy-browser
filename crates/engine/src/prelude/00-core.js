// The scope the rest of the prelude shares.
//
// The prelude is several standalone scripts, evaluated in filename order into
// one global scope. What would be closure state in a single script is named
// state on `__tb`; only what a page is meant to find reaches `globalThis`.
//
// The wrapper cache and the interface table live in Rust now — see
// `realm/node/support.rs`. What remains here is the name the rest of the
// prelude reaches for.

globalThis.__tb = {
  // One wrapper per node id, so `a === b` holds for the same element. Minting
  // and remembering both happen on the Rust side.
  wrap: __dom.wrap,
};
