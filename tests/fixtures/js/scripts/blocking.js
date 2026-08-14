// Runs during head parsing, before <body> exists.
window.__loadOrder = ["blocking"];

document.addEventListener("DOMContentLoaded", () => {
  const stage = document.getElementById("stage");
  for (const name of window.__loadOrder) {
    const row = document.createElement("div");
    row.className = "row";
    row.textContent = `${name} script ran`;
    stage.appendChild(row);
  }
});
