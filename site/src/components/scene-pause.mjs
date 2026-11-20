// The motion of the hero's drawing, the one drawing on the page that moves (the
// scenario's stand still, NOC-380). Each button[data-scene-pause] pauses and plays
// every drawing inside its [data-scene-scope]. A drawing is still (held at STILL_AT,
// then paused) under reduced motion and in a screenshot (?shot), and its control is
// hidden there. A phone does not show the hero's drawing at all (landing-page.css).
// The markup carries every button hidden, so without script none stands dead; apply()
// shows it wherever the drawing can move.

// 0.72 s is 0.2 of the drawing's 3.6 s cycle: in each state with travelling points
// one of them is half-way along its line (scene.mjs, pulse windows), and the agent's
// circling point is visible at any time.
const STILL_AT = 0.72;
const reduced =
  matchMedia("(prefers-reduced-motion: reduce)").matches ||
  document.documentElement.dataset.shot !== undefined;

const hold = (svg) => {
  svg.setCurrentTime(STILL_AT);
  svg.pauseAnimations();
};

for (const button of document.querySelectorAll("[data-scene-pause]")) {
  const scope = button.closest("[data-scene-scope]");
  let paused = false;
  const apply = () => {
    button.hidden = reduced;
    for (const svg of scope.querySelectorAll("svg.sc")) {
      if (reduced) hold(svg);
      else if (paused) svg.pauseAnimations();
      else svg.unpauseAnimations();
    }
  };
  button.addEventListener("click", () => {
    // One name and one word, Pause; aria-pressed (and its look) says whether it holds.
    paused = !paused;
    button.setAttribute("aria-pressed", String(paused));
    apply();
  });
  apply();
}
