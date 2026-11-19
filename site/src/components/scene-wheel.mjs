// A gesture lock for the pinned scenario (NOC-360, the owner's point 9 of
// 2026-11-19), in the spirit of Lenis but for this one section: while the
// scenario is pinned, one wheel or trackpad gesture moves it by one step at
// most. A gesture is a run of wheel events with less than QUIET ms between
// them, its momentum tail included, so a hard flick still stops at the next
// step. The step change is eased toward its target, a fraction per frame.
//
// It never traps the reader: a gesture that would move past the first or the
// last step scrolls natively, out of the scenario. It leaves alone the
// keyboard, the scrollbar, touch (which sends no wheel events), pinch zoom
// (a wheel event with ctrlKey), a phone or short window (the scenario is not
// live there) and reduced motion.
const QUIET = 150;
const EASE = 0.18;

const story = document.getElementById("story");
const steps = story ? story.querySelectorAll(".mo-pc__step").length : 0;
const reduced = matchMedia("(prefers-reduced-motion: reduce)");

/** The scenario's top on the page, and the scroll one step holds (a screen). */
const geometry = () => {
  const rect = story.getBoundingClientRect();
  const span = (rect.height - innerHeight) / steps;
  return { top: rect.top + scrollY, rect, span };
};

/** Whether the scenario is pinned now: its stage holds the screen. */
const pinned = ({ rect }) => rect.top <= 1 && rect.bottom >= innerHeight - 1;

/** The open step now, 0 to steps - 1. */
const stepNow = ({ top, span }) =>
  Math.min(steps - 1, Math.max(0, Math.floor((scrollY - top) / span)));

let last = 0; // the time of the gesture's last wheel event
let mode = "none"; // this gesture: "lock" (held by the scenario) or "native"
let target = null; // the scroll position the eased step change moves to
let frame = 0;
const root = document.documentElement;

const ease = () => {
  frame = 0;
  if (target === null) return;
  const gap = target - scrollY;
  if (Math.abs(gap) < 2) {
    scrollTo(0, target);
    target = null;
    return;
  }
  // at least a pixel a frame: a fraction of a small gap rounds to no move at all
  const move = gap * EASE;
  scrollTo(0, scrollY + Math.sign(move) * Math.max(1, Math.abs(move)));
  frame = requestAnimationFrame(ease);
};

/** Moves to the middle of `step`'s screen of scroll, eased. */
const goTo = (step, g) => {
  target = Math.round(g.top + (step + 0.5) * g.span);
  if (!frame) frame = requestAnimationFrame(ease);
};

const onWheel = (event) => {
  if (event.ctrlKey || reduced.matches || !story.classList.contains("is-live"))
    return;
  // A wheel user: the lock takes the steps over from the CSS snap points, which
  // would pull each eased frame back, and pull a reader leaving the scenario
  // back to its last step. Touch sends no wheel events and keeps them.
  root.style.scrollSnapType = "none";
  const now = event.timeStamp;
  const fresh = now - last > QUIET;
  last = now;
  const g = geometry();
  if (fresh) {
    mode = "none";
    if (pinned(g)) {
      const next = stepNow(g) + Math.sign(event.deltaY);
      if (next < 0 || next >= steps) {
        // out of the scenario: native scrolling, and no eased move holds it back
        mode = "native";
        target = null;
      } else {
        mode = "lock";
        goTo(next, g);
      }
    }
  } else if (mode === "none" && pinned(g)) {
    // a gesture from outside reached the scenario: it stops at the step it entered
    mode = "lock";
    goTo(stepNow(g), g);
  }
  if (mode === "lock") event.preventDefault();
};

if (story && steps > 1) addEventListener("wheel", onWheel, { passive: false });
