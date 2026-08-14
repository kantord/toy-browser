// A bare specifier: only resolvable once the import map has been read.
import { swatches } from "palette";

const stage = document.getElementById("stage");
for (const { label, color } of swatches) {
  const swatch = document.createElement("div");
  swatch.className = "swatch";
  swatch.style.background = color;
  swatch.textContent = label;
  stage.appendChild(swatch);
}
