// One account of a document, taken the same way from whichever browser is
// asked. Shared rather than copied, because "both sides are in one format by
// construction" stops being true the moment there are two copies of this.

/**
 * Every element, named by where it sits in the tree rather than by anything it
 * says — so two browsers describing the same document produce the same keys,
 * and a value that differs is a real disagreement rather than a mismatch.
 */
export const EXPORT = () => {
  const nodes = [];
  const visit = (element, path) => {
    const box = element.getBoundingClientRect();
    // Own text, not `textContent`: a leaf that disagrees would otherwise make
    // every ancestor disagree with it, and a report would name the document
    // rather than the word.
    let own = "";
    const children = element.childNodes;
    for (let i = 0; i < children.length; i += 1) {
      if (children[i].nodeType === 3) own += children[i].nodeValue ?? "";
    }
    nodes.push({
      path,
      tag: element.tagName,
      id: element.id || null,
      text: own.replace(/\s+/g, " ").trim().slice(0, 60),
      rect: [box.x, box.y, box.width, box.height].map(
        (n) => Math.round(n * 100) / 100,
      ),
    });
    const elements = element.children;
    for (let i = 0; i < elements.length; i += 1) visit(elements[i], `${path}/${i}`);
  };
  visit(document.documentElement, "0");
  return { url: location.href, title: document.title, nodes };
};

/**
 * The page as one self-contained file: the DOM after its scripts have run, with
 * every stylesheet inlined and a `base` so its own references still resolve.
 *
 * Taken from the reference browser, so what gets reduced is a document both
 * browsers agree exists, rather than markup one of them had to guess at.
 */
export const FREEZE = () => {
  const css = [...document.styleSheets]
    .map((sheet) => {
      try {
        return [...sheet.cssRules].map((rule) => rule.cssText).join("\n");
      } catch {
        return ""; // A sheet from another origin will not open. Skip it.
      }
    })
    .join("\n");
  return `<!DOCTYPE html><html><head><meta charset="utf-8">
<base href="${location.href}">
<style>${css}</style></head>
<body>${document.body.innerHTML}</body></html>`;
};
