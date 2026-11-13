// The motion of the landing page's drawings.
// Each button[data-scene-pause] pauses and plays every drawing inside its
// [data-scene-scope]. A drawing is still (held at STILL_AT, then paused) under reduced
// motion, in a screenshot (?shot), and in the scenario while its moment is at rest
// (tablets, short screens: no is-live). There the control is hidden or out of view.
// On a phone a drawing plays its loop once each time it scrolls into view and then
// rests on the same picture: under five seconds of motion, so it needs no control.
// The markup carries every button hidden, so without script none stands dead; apply()
// shows it wherever the drawing can move.

// 0.72 s is 0.2 of the drawing's 3.6 s cycle: in each state with travelling points
// one of them is half-way along its line (scene.mjs, pulse windows), and the agent's
// circling point is visible at any time.
const STILL_AT = 0.72;
const reduced =
  matchMedia("(prefers-reduced-motion: reduce)").matches ||
  document.documentElement.dataset.shot !== undefined;
const phone = matchMedia("(max-width: 639px)");

// One pass on a phone: from the start of the loop to the still picture one loop later.
const ONCE_MS = (3.6 + STILL_AT) * 1000;
const timers = new WeakMap();
const hold = (svg) => {
  clearTimeout(timers.get(svg));
  svg.setCurrentTime(STILL_AT);
  svg.pauseAnimations();
};
const playOnce = (svg) => {
  clearTimeout(timers.get(svg));
  svg.setCurrentTime(0);
  svg.unpauseAnimations();
  timers.set(
    svg,
    setTimeout(() => hold(svg), ONCE_MS),
  );
};
// Whether a phone's tall drawings are played by the observer below. Where they are
// not (reduced motion, no IntersectionObserver), apply() holds them like the rest.
const watching = !reduced && "IntersectionObserver" in window;
if (watching) {
  const seen = new IntersectionObserver(
    (entries) => {
      if (!phone.matches) return;
      for (const e of entries) (e.isIntersecting ? playOnce : hold)(e.target);
    },
    { threshold: 0.6 },
  );
  for (const svg of document.querySelectorAll("svg.sc--tall"))
    seen.observe(svg);
}

for (const button of document.querySelectorAll("[data-scene-pause]")) {
  const scope = button.closest("[data-scene-scope]");
  const moment = scope.matches("[data-moment]") ? scope : null;
  let paused = false;
  const apply = () => {
    const still =
      reduced ||
      phone.matches ||
      (moment && !moment.classList.contains("is-live"));
    button.hidden = reduced || phone.matches;
    for (const svg of scope.querySelectorAll("svg.sc")) {
      if (svg.classList.contains("sc--tall")) {
        // Shown on a phone only, where the observer plays it; hidden and held elsewhere.
        if (!(watching && phone.matches)) hold(svg);
        continue;
      }
      if (still) hold(svg);
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
