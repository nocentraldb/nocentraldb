// The pause control of the landing page's drawings: each button[data-scene-pause]
// pauses and plays every drawing inside its [data-scene-scope]. Under reduced
// motion and in a screenshot (?shot) the drawings start paused and the control is hidden.
const still =
  matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.shot !== undefined;

for (const button of document.querySelectorAll("[data-scene-pause]")) {
  const scope = button.closest("[data-scene-scope]");
  const set = (paused) => {
    for (const svg of scope.querySelectorAll("svg.sc")) paused ? svg.pauseAnimations() : svg.unpauseAnimations();
    button.textContent = paused ? "Play" : "Pause";
    button.setAttribute("aria-pressed", String(paused));
  };
  if (still) {
    set(true);
    button.hidden = true;
  } else {
    button.addEventListener("click", () => set(button.getAttribute("aria-pressed") !== "true"));
  }
}
