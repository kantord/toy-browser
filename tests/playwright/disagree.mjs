// How two accounts of one document differ, as lines anybody can read.
//
// Shared rather than copied, for the reason `export.mjs` gives about itself:
// the corpus compares one page at one size and the zoom spec compares three
// pages at six, and "both are measuring the same thing" stops being true the
// moment there are two of this.

const apart = (ours, theirs) =>
  Math.max(...[0, 1, 2, 3].map((at) => Math.abs(ours.rect[at] - theirs.rect[at])));

/**
 * How the two accounts of one case differ, as lines anybody can read, under a
 * total.
 *
 * The total is what makes the ratchet see an improvement that fixes nothing
 * outright: registering fonts took several gaps from 4px to 1px and left the
 * number of disagreeing elements exactly where it was, so counting them alone
 * reported no change at all.
 */
export function disagreements(ours, theirs) {
  const mine = new Map(ours.nodes.map((node) => [node.path, node]));
  let total = 0;
  const lines = theirs.nodes
    .map((node) => {
      const ours = mine.get(node.path);
      if (!ours) return `${node.tag} ${node.path}  missing here`;
      if (ours.tag !== node.tag) return `${node.path}  we say ${ours.tag}, they say ${node.tag}`;
      if (JSON.stringify(ours.rect) === JSON.stringify(node.rect)) return null;
      const off = apart(ours, node);
      total += off;
      return `${node.tag} ${node.path}  off by ${off.toFixed(2)}px  ours ${JSON.stringify(ours.rect)}  theirs ${JSON.stringify(node.rect)}`;
    })
    .filter(Boolean);

  if (lines.length === 0) return [];
  return [`${lines.length} elements, ${total.toFixed(2)}px apart in total`, ...lines];
}
