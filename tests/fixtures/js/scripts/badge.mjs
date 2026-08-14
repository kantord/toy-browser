export function badge(stage) {
  const swatch = document.createElement("div");
  swatch.className = "swatch";
  swatch.style.background = "#fde68a";
  swatch.textContent = "inline module ran";
  stage.appendChild(swatch);
}
