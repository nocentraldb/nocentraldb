// The motion of the scenario's drawings (owner, 2026-11-20, NOC-384): each step's
// wide drawing plays its loop on its own, in place. It waits at the start of its loop
// until it comes into view, then plays by itself; out of view it holds where it is
// and plays on when it comes back. Scrolling only decides whether a drawing is
// visible, never how far it has played: nothing here reads the scroll position.
// Under reduced motion (or in a screenshot, ?shot) every drawing holds the picture
// 0.72 s into its loop, the still picture NOC-380 showed, and no control shows.
// One button[data-story-pause] per figure; any of them holds or releases all four.
// The hero's drawing has its own control (scene-pause.mjs).

// 0.72 s is 0.2 of the drawing's 3.6 s loop (scene.mjs, STILL_AT).
const STILL_AT = 0.72;

/** Wires the drawings and controls inside root. */
export function wire(root, { reduced, Observer }) {
  const figures = [...root.querySelectorAll("figure")];
  const buttons = [...root.querySelectorAll("[data-story-pause]")];
  const drawing = (figure) => figure.querySelector("svg.sc--wide");
  const inView = new Set();
  let paused = false;

  for (const figure of figures) {
    const svg = drawing(figure);
    svg.setCurrentTime(reduced ? STILL_AT : 0);
    svg.pauseAnimations();
  }
  for (const button of buttons) button.hidden = reduced;

  const apply = () => {
    for (const figure of figures) {
      if (!reduced && !paused && inView.has(figure))
        drawing(figure).unpauseAnimations();
      else drawing(figure).pauseAnimations();
    }
  };
  for (const button of buttons) {
    button.addEventListener("click", () => {
      paused = !paused;
      for (const b of buttons) b.setAttribute("aria-pressed", String(paused));
      apply();
    });
  }
  const observer = new Observer((entries) => {
    for (const { target, isIntersecting } of entries) {
      if (isIntersecting) inView.add(target);
      else inView.delete(target);
    }
    apply();
  });
  for (const figure of figures) observer.observe(figure);
}
