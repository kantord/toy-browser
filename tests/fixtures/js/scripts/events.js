const add = (text) => {
  const line = document.createElement("div");
  line.className = "line";
  line.textContent = text;
  document.getElementById("stage").appendChild(line);
};

document.addEventListener("readystatechange", () => add(`readyState: ${document.readyState}`));
document.addEventListener("DOMContentLoaded", () => add("DOMContentLoaded fired"));
window.addEventListener("load", () => add("window load fired"));
window.addEventListener("error", () => add("subresource error observed"), true);
