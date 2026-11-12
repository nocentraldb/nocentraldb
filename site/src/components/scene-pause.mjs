// The pause control of the landing page's drawings: each button[data-scene-pause]
// pauses and plays every drawing inside its [data-scene-scope]. A drawing is still
// (held at STILL_AT, then paused) under reduced motion, in a screenshot (?shot), on a
// phone, and in the scenario while its moment is at rest (tablets, short screens: no
// is-live). There the control is hidden or out of view, so no button is needed.

// 0.72 s is 0.2 of the drawing's 3.6 s cycle: in each state with travelling points
// one of them is half-way along its line (scene.mjs, pulse windows), and the agent's
// circling point is visible at any time.
const STILL_AT = 0.72;
const reduced =
  matchMedia("(prefers-reduced-motion: reduce)").matches || document.documentElement.dataset.shot !== undefined;
const phone = matchMedia("(max-width: 639px)");

for (const button of document.querySelectorAll("[data-scene-pause]")) {
  const scope = button.closest("[data-scene-scope]");
  const moment = scope.matches("[data-moment]") ? scope : null;
  let paused = false;
  const apply = () => {
    const still = reduced || phone.matches || (moment && !moment.classList.contains("is-live"));
    button.hidden = reduced || phone.matches;
    for (const svg of scope.querySelectorAll("svg.sc")) {
      if (still) {
        svg.setCurrentTime(STILL_AT);
        svg.pauseAnimations();
      } else if (paused) svg.pauseAnimations();
      else svg.unpauseAnimations();
    }
  };
  button.addEventListener("click", () => {
    // One name and one word, Pause; aria-pressed (and its look) says whether it holds.
    paused = !paused;
    button.setAttribute("aria-pressed", String(paused));
    apply();
  });
  phone.addEventListener("change", apply);
  // The moment goes live and back with a class, and moves the figures into and out of
  // its shared frame; a moved SVG restarts its timeline, so the state is applied again.
  if (moment)
    new MutationObserver(apply).observe(moment, {
      attributes: true,
      attributeFilter: ["class"],
      childList: true,
      subtree: true,
    });
  apply();
}
