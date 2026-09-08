// `localStorage` and `sessionStorage`.
//
// Held in memory for as long as the realm is, and shared with nothing. A real
// browser keeps `localStorage` on disk per origin and `sessionStorage` per tab;
// this browser renders a page once and throws the realm away, so the two are
// the same store built twice, and the difference between them has nowhere to
// show itself.
//
// The reason to have them at all is not that pages read what they wrote. It is
// that pages ask whether they *exist*. MediaWiki opens with
// `'localStorage' in window` as part of a compatibility test, and a browser
// that fails it is served the no-JavaScript site: on Wikipedia that meant
// `client-js` being reverted to `client-nojs`, every collapsed navbox opening,
// and the article coming out 20000px too tall. Nothing read a key. The feature
// test was the whole of it.

(() => {
  const make = () => {
    const held = new Map();
    // Web Storage stringifies everything on the way in, both keys and values,
    // so `setItem("a", 1)` reads back as `"1"` and not as a number.
    const api = {
      getItem: (key) => (held.has(String(key)) ? held.get(String(key)) : null),
      setItem: (key, value) => void held.set(String(key), String(value)),
      removeItem: (key) => void held.delete(String(key)),
      clear: () => held.clear(),
      // Insertion order, which is what a Map iterates in. The standard leaves
      // the order to the implementation, so any consistent one is an answer.
      key: (index) => [...held.keys()][index] ?? null,
    };
    Object.defineProperty(api, "length", { get: () => held.size });

    // A Storage is also a plain object: `storage.token` reads the key `token`,
    // and assigning to it writes one. A Proxy is the honest way to say that,
    // since the set of keys is whatever a page has put there.
    return new Proxy(api, {
      get: (target, name) =>
        name in target || typeof name === "symbol" ? target[name] : api.getItem(name),
      set: (target, name, value) => {
        if (name in target) return false;
        api.setItem(name, value);
        return true;
      },
      has: (target, name) => name in target || held.has(String(name)),
      deleteProperty: (target, name) => {
        held.delete(String(name));
        return true;
      },
      ownKeys: () => [...held.keys()],
      getOwnPropertyDescriptor: (target, name) =>
        held.has(String(name))
          ? { value: held.get(String(name)), writable: true, enumerable: true, configurable: true }
          : Object.getOwnPropertyDescriptor(target, name),
    });
  };

  globalThis.localStorage = make();
  globalThis.sessionStorage = make();
})();
