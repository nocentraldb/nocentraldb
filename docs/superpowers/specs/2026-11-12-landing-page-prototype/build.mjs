// Throwaway prototype: the scroll scenario for nocentraldb.io (train, home office, agent, cloud server).
// node build.mjs  ->  index.html
import { writeFileSync } from "node:fs";

const W = 230;
const X = { train: 20, home: 285, agent: 550 };
const CARD_Y = 236;
const LINK = {
  train: "M135 236 C135 186 360 200 360 150",
  home: "M400 236 V150",
  agent: "M665 236 C665 186 440 200 440 150",
};
const reverse = {
  train: "M360 150 C360 200 135 186 135 236",
  home: "M400 150 V236",
  agent: "M440 150 C440 200 665 186 665 236",
};

const pulse = (d, t0, t1) => `
  <circle r="6" class="sc-pulse" opacity="0">
    <animateMotion dur="3.6s" repeatCount="indefinite" calcMode="linear" keyPoints="0;0;1;1" keyTimes="0;${t0};${t1};1" path="${d}"/>
    <animate attributeName="opacity" dur="3.6s" repeatCount="indefinite" values="0;0;1;1;0;0" keyTimes="0;${t0};${(t0 + 0.01).toFixed(2)};${(t1 - 0.01).toFixed(2)};${t1};1"/>
  </circle>`;

const card = (who, label, rows, { offline = false, loop = false } = {}) => {
  const x = X[who];
  const y = CARD_Y;
  const h = 44 + rows.length * 50 + 8;
  const perimeter = `M${x + 14} ${y} H${x + W - 14} a14 14 0 0 1 14 14 V${y + h - 14} a14 14 0 0 1 -14 14 H${x + 14} a14 14 0 0 1 -14 -14 V${y + 14} a14 14 0 0 1 14 -14 Z`;
  return `
  <g>
    <rect x="${x}" y="${y}" width="${W}" height="${h}" rx="14" class="sc-card"/>
    <text x="${x + 16}" y="${y + 29}" class="sc-label">${label}</text>
    <circle cx="${x + W - 22}" cy="${y + 23}" r="5" class="${offline ? "sc-dot-off" : "sc-dot-on"}"/>
    ${rows
      .map(([title, status, kind], i) => {
        const ry = y + 44 + i * 50;
        return `
    <rect x="${x + 8}" y="${ry}" width="${W - 16}" height="44" rx="9" class="sc-row ${kind ? `sc-row-${kind}` : ""}"/>
    <text x="${x + 22}" y="${ry + 20}" class="sc-title ${kind === "origin" ? "sc-on-accent" : ""}">${title}</text>
    <text x="${x + 22}" y="${ry + 36}" class="sc-status ${kind === "origin" ? "sc-on-accent" : ""}">${status}</text>`;
      })
      .join("")}
    ${
      loop
        ? `<circle r="5" class="sc-pulse"><animateMotion dur="1.4s" repeatCount="indefinite" path="${perimeter}"/></circle>`
        : ""
    }
  </g>`;
};

const cloud = (lens) => `
  <path d="M312 150 H490 a44 44 0 0 0 6 -87 a58 58 0 0 0 -108 -20 a50 50 0 0 0 -92 26 a41 41 0 0 0 16 81 Z" class="sc-cloud"/>
  <text x="400" y="92" text-anchor="middle" class="sc-label">Cloud server</text>
  <text x="400" y="116" text-anchor="middle" class="sc-cipher">9f3a c01e 77b2 d54a</text>
  <text x="400" y="134" text-anchor="middle" class="sc-cipher">0c6e 1f8b 3d92 a7e4</text>
  ${
    lens
      ? `
  <line x1="573" y1="137" x2="604" y2="168" class="sc-handle"/>
  <circle cx="528" cy="92" r="64" class="sc-lens"/>
  <clipPath id="lens-clip"><circle cx="528" cy="92" r="60"/></clipPath>
  <g clip-path="url(#lens-clip)">
    <text x="528" y="68" text-anchor="middle" class="sc-cipher-big">e77b2d54</text>
    <text x="528" y="100" text-anchor="middle" class="sc-cipher-big">a0c6e1f8</text>
    <text x="528" y="132" text-anchor="middle" class="sc-cipher-big">b3d92a7e</text>
  </g>`
      : ""
  }`;

const train = (tx, clipX) => `
  <g ${clipX ? `clip-path="url(#train-clip-${clipX})"` : ""}>
    ${clipX ? `<clipPath id="train-clip-${clipX}"><rect x="0" y="440" width="${clipX}" height="160"/></clipPath>` : ""}
    <g transform="translate(${tx} 0)">
      <rect x="0" y="506" width="150" height="48" rx="13" class="sc-thing"/>
      <rect x="14" y="517" width="30" height="18" rx="5" class="sc-window"/>
      <rect x="56" y="517" width="30" height="18" rx="5" class="sc-window sc-window-lit"/>
      <rect x="98" y="517" width="30" height="18" rx="5" class="sc-window"/>
      <circle cx="32" cy="558" r="7" class="sc-thing"/><circle cx="118" cy="558" r="7" class="sc-thing"/>
    </g>
  </g>`;

// The hill with its tunnel mouth. "ahead": to the right of the train, mouth facing it.
// "behind": mirrored at the left edge, the train has come out of it.
const hill = (where) => `
  <g ${where === "behind" ? 'transform="translate(304 0) scale(-1 1)"' : ""}>
    <path d="M176 566 L196 500 L228 462 L258 486 L284 468 L310 524 L318 566 Z" class="sc-hill"/>
    <path d="M190 566 V524 a30 30 0 0 1 60 0 V566 Z" class="sc-mouth"/>
  </g>`;

const scenery = (tunnel) => `
  <line x1="0" y1="566" x2="322" y2="566" class="sc-track"/>
  ${hill(tunnel === "out" ? "behind" : "ahead")}
  ${tunnel === "in" ? train(150, 221) : tunnel === "out" ? train(150) : train(20)}
  <path d="M340 566 V508 L405 464 L470 508 V566 Z" class="sc-thing"/>
  <rect x="389" y="518" width="32" height="28" rx="5" class="sc-window sc-window-lit"/>
  <rect x="595" y="482" width="140" height="84" rx="12" class="sc-thing"/>
  <line x1="595" y1="510" x2="735" y2="510" class="sc-track"/><line x1="595" y1="538" x2="735" y2="538" class="sc-track"/>
  <circle cx="614" cy="496" r="4" class="sc-lamp"/><circle cx="614" cy="524" r="4" class="sc-lamp"/><circle cx="614" cy="552" r="4" class="sc-lamp"/>`;

const link = (who, cut) => `<path d="${LINK[who]}" class="sc-link ${cut ? "sc-link-cut" : ""}"/>`;

const svg = (alt, body) =>
  `<svg class="sc" viewBox="0 0 800 600" role="img" aria-label="${alt}">${body}</svg>`;

const R = {
  readme: (s, k) => ["Write README", s, k],
  rfc: (s, k) => ["Review RFC", s, k],
  release: (s, k) => ["Plan release", s, k],
};

const scenes = [
  svg(
    "A laptop on a train, a desk in a home office and an agent each hold the same three tasks. A cloud server sits between them.",
    `${link("train")}${link("home")}${link("agent")}${cloud(false)}
     ${card("train", "On the train", [R.readme("open"), R.rfc("open"), R.release("open")])}
     ${card("home", "Home office", [R.readme("open"), R.rfc("open"), R.release("open")])}
     ${card("agent", "Agent", [R.readme("open"), R.rfc("open"), R.release("open")], { loop: true })}
     ${scenery()}`,
  ),
  svg(
    "The train changes one task. The change travels through the cloud server to the home office and the agent. A lens on the server shows only ciphertext.",
    `${link("train")}${link("home")}${link("agent")}${cloud(true)}
     ${pulse(LINK.train, 0.05, 0.4)}${pulse(reverse.home, 0.45, 0.8)}${pulse(reverse.agent, 0.45, 0.8)}
     ${card("train", "On the train", [R.readme("in progress", "origin"), R.rfc("open"), R.release("open")])}
     ${card("home", "Home office", [R.readme("in progress", "got"), R.rfc("open"), R.release("open")])}
     ${card("agent", "Agent", [R.readme("in progress", "got"), R.rfc("open"), R.release("open")])}
     ${scenery()}`,
  ),
  svg(
    "The train is in a tunnel and offline. Its change is stored locally. The home office and the agent keep exchanging changes.",
    `${link("train", true)}${link("home")}${link("agent")}${cloud(false)}
     ${pulse(LINK.home, 0.05, 0.3)}${pulse(reverse.agent, 0.32, 0.57)}${pulse(LINK.agent, 0.6, 0.8)}${pulse(reverse.home, 0.82, 0.98)}
     ${card("train", "On the train", [R.readme("in progress"), R.rfc("done · stored locally", "origin"), R.release("open")], { offline: true })}
     ${card("home", "Home office", [R.readme("in progress"), R.rfc("open"), R.release("assigned to agent", "origin")])}
     ${card("agent", "Agent", [R.readme("in progress"), R.rfc("open"), R.release("assigned to agent", "got")], { loop: true })}
     ${scenery("in")}`,
  ),
  svg(
    "The train has left the tunnel. Changes cross in both directions and all three copies show the same tasks.",
    `${link("train")}${link("home")}${link("agent")}${cloud(false)}
     ${pulse(LINK.train, 0.05, 0.3)}${pulse(reverse.home, 0.32, 0.57)}${pulse(reverse.agent, 0.32, 0.57)}${pulse(reverse.train, 0.6, 0.9)}
     ${card("train", "On the train", [R.readme("in progress"), R.rfc("done"), R.release("assigned to agent", "got")])}
     ${card("home", "Home office", [R.readme("in progress"), R.rfc("done", "got"), R.release("assigned to agent")])}
     ${card("agent", "Agent", [R.readme("in progress"), R.rfc("done", "got"), R.release("assigned to agent")])}
     ${scenery("out")}`,
  ),
];

const steps = [
  [
    "Everyone holds a full copy",
    "The laptop on the train, the desk at home and the agent each keep the whole task list on their own disk. The agent reads its copy as often as it likes, and no read crosses the network.",
  ],
  [
    "A change travels as ciphertext",
    "The train marks a task as in progress. The change goes through a server in the cloud to the other two. Hold a lens over that server and you see bytes it has no key for.",
  ],
  [
    "The train enters a tunnel",
    "The laptop is offline and its owner keeps working; the change is stored locally. At home and with the agent the work goes on too. They don't see the train's change yet.",
  ],
  [
    "Out of the tunnel, every copy matches",
    "The laptop reconnects. Its change goes out, the others' changes come in, and all three copies end in the same state.",
  ],
];

const html = `<!doctype html>
<html lang="en" data-type="software" data-radius="soft" data-buttons="pill" data-motion>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>NoCentralDB prototype: the scroll scenario</title>
<meta name="site-kind" content="software"><meta name="page-type" content="software">
<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Encode+Sans+Semi+Expanded:wght@700&family=Commissioner:wght@350..700&family=Syne+Mono&display=swap" rel="stylesheet">
<link rel="stylesheet" href="assets/system.css">
<link rel="stylesheet" href="assets/ui/cards.css">
<link rel="stylesheet" href="assets/motion/motion.css">
<link rel="stylesheet" href="assets/backgrounds/backgrounds.css">
<link rel="stylesheet" href="assets/layout/layout.css">
<style>
:root {
  --p-bg: #111110; --p-surface: #1b1b19; --p-ink: #ecebe4; --p-muted: #a3a299;
  --p-accent: #f2dc4b; --p-accent-ink: #111110; --p-action: #f2dc4b; --p-action-ink: #111110;
  --p-inv-bg: #ecebe4; --p-inv-surface: #dad9d3; --p-inv-ink: #111110; --p-inv-muted: #5a5a57;
  --p-inv-accent: #111110; --p-inv-accent-ink: #ecebe4; --p-inv-action: #111110; --p-inv-action-ink: #ecebe4;
  --font-display: "Encode Sans Semi Expanded", system-ui, sans-serif;
  --font-text: "Commissioner", system-ui, sans-serif;
  --font-mono: "Syne Mono", ui-monospace, Menlo, monospace;
  --display-weight: 700; --display-tracking: -0.015em; --display-tracking-xl: -0.015em; --ly-scale: 0.9;
}
body { font-weight: 350; }
@media (max-height: 780px) and (min-width: 1024px) { .hero h1 { font-size: clamp(36px, 7.6svh, 60px); } .hero .hero__lead { margin-top: 16px; } .status-line { margin-top: 16px !important; } }
.status-line { color: var(--muted); font-size: var(--t-small); max-width: 44ch; margin-top: var(--s-5); }
.proto { position: fixed; z-index: 99; left: 12px; bottom: 12px; padding: 6px 12px; border-radius: 999px; background: #26261f; color: #a3a299; font: 12px var(--font-mono); }
.scene-frame { border-radius: var(--r-lg); overflow: hidden; background: #1b1b19; aspect-ratio: 4 / 3; }
.mo-pc__visual { background: #1b1b19; }
.sc { display: block; width: 100%; height: 100%; }
.sc text { font-family: "Commissioner", system-ui, sans-serif; fill: #ecebe4; }
.sc .sc-label { font-size: 17px; font-weight: 600; }
.sc .sc-title { font-size: 17px; font-weight: 500; }
.sc .sc-status { font-size: 13.5px; fill: #a3a299; }
.sc .sc-on-accent { fill: #111110; }
.sc .sc-cipher { font-family: "Syne Mono", monospace; font-size: 13px; fill: #a3a299; }
.sc .sc-cipher-big { font-family: "Syne Mono", monospace; font-size: 26px; fill: #ecebe4; }
.sc-card { fill: #26261f; }
.sc-row { fill: #111110; }
.sc-row-origin { fill: #f2dc4b; }
.sc-row-got { stroke: #f2dc4b; stroke-width: 1.5; }
.sc-cloud { fill: #26261f; }
.sc-link { fill: none; stroke: #a3a299; stroke-opacity: 0.45; stroke-width: 2; }
.sc-link-cut { stroke-dasharray: 3 9; stroke-opacity: 0.3; }
.sc-pulse { fill: #f2dc4b; }
.sc-dot-on { fill: #ff3d8b; }
.sc-lamp { fill: #a3a299; }
.band-yellow { background: #f2dc4b; color: #111110; }
.band-yellow .btn--primary { background: #111110; color: #ecebe4; }
.band-yellow h2 { color: #111110; }
.doc-cards { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; margin: 48px 0 40px; }
.doc-cards a { display: grid; gap: 8px; padding: 24px 22px 28px; border-radius: 16px; background: #1b1b19; color: #ecebe4; text-decoration: none; }
.doc-cards strong { font: 700 22px var(--font-display); }
.doc-cards span { color: #a3a299; }
@media (max-width: 900px) { .doc-cards { grid-template-columns: 1fr 1fr; } }
.sc-dot-off { fill: none; stroke: #a3a299; stroke-width: 1.5; }
.sc-thing { fill: #26261f; stroke: #a3a299; stroke-opacity: 0.6; stroke-width: 2; }
.sc-window { fill: #111110; }
.sc-window-lit { fill: #ecebe4; fill-opacity: 0.85; }
.sc-track { stroke: #a3a299; stroke-opacity: 0.4; stroke-width: 2; }
.sc-hill { fill: #2f2f27; }
.sc-mouth { fill: #111110; }
.nav__logo img { height: 44px; width: auto; display: block; }
.sc-lens { fill: #111110; stroke: #ecebe4; stroke-width: 5; }
.sc-handle { stroke: #ecebe4; stroke-width: 10; stroke-linecap: round; }
</style>
<script>document.documentElement.classList.add('m-enter');setTimeout(()=>document.documentElement.classList.add('m-enter-done'),700)</script>
<script>document.documentElement.classList.add('mo-js');setTimeout(()=>document.documentElement.classList.add('mo-safe'),2500)</script>
<link rel="modulepreload" href="assets/moments/moments.js"><link rel="modulepreload" href="assets/moments/lib.js">
<link rel="modulepreload" href="assets/moments/pinned-chapters/moment.js">
</head>
<body>
<span class="proto">throwaway prototype</span>
<header class="nav">
  <div class="wrap nav__bar">
    <a class="skip-link" href="#top">Skip to content</a>
    <a class="nav__logo" href="#top"><img src="assets/ncdb-wordmark-dark.svg" alt="NoCentralDB"></a>
    <nav class="nav__links" aria-label="Main">
      <div class="nav__item"><a class="nav__link" href="#story">How it works</a></div>
      <div class="nav__item"><a class="nav__link" href="#code">Code</a></div>
    </nav>
    <div class="nav__actions"><a class="btn btn--primary" href="#code">Read the docs</a></div>
  </div>
</header>
<main id="top">
  <section class="hero hero--split" data-enter aria-labelledby="hero-h">
    <div class="wrap"><div class="hero-text grid hero__grid">
      <div class="hero__text span-6">
        <p class="hero__kicker"><span class="dot" aria-hidden="true"></span>A local-first database for people and agents</p>
        <h1 id="hero-h">A database that reads like a local file and syncs like a shared doc</h1>
        <p class="hero__lead">Each person and each agent reads and writes a full copy on their own machine. The changes sync encrypted through a server that can't read them.</p>
        <div class="btn-row"><a class="btn btn--primary" href="#story">See how it works</a><a class="btn btn--secondary" href="#code">Review the RFCs</a></div>
        <p class="status-line">NoCentralDB does not exist yet. This page shows what it is designed to do.</p>
      </div>
      <div class="span-6 start-7"><div class="scene-frame">${scenes[0]}</div></div>
    </div></div>
  </section>

  <section class="section" id="story" style="--h: 2" data-moment="pinned-chapters" aria-labelledby="story-h">
    <div class="wrap mo-pc__grid">
      <div class="mo-pc__text">
        <h2 id="story-h">One task list on a train, in a home office and with an agent</h2>
        <ol class="mo-pc__steps">
          ${steps
            .map(
              ([t, p], i) => `<li class="mo-pc__step">
            <h3>${t}</h3>
            <p>${p}</p>
            <figure class="mo-pc__visual">${scenes[i]}</figure>
          </li>`,
            )
            .join("\n          ")}
        </ol>
      </div>
      <div class="mo-pc__visuals frame"></div>
    </div>
  </section>

  <section class="section" id="code" aria-labelledby="code-h">
    <div class="wrap">
      <div class="section-head">
        <h2 id="code-h">Five calls in the SDK</h2>
        <p>Placeholder: the code tabs and the closing section follow here in the real page.</p>
      </div>
    </div>
  </section>
  <section class="section band-yellow" id="docs" aria-labelledby="docs-h">
    <div class="wrap">
      <h2 id="docs-h">Read the design, then tell us where it breaks</h2>
      <div class="doc-cards">
        <a href="#top"><strong>Getting started</strong><span>Install, describe a model, open a database.</span></a>
        <a href="#top"><strong>Concepts</strong><span>Spaces, the relay, convergence, shredding.</span></a>
        <a href="#top"><strong>API</strong><span>The SDK calls, as RFC 0009 designs them.</span></a>
        <a href="#top"><strong>RFCs</strong><span>The whole design, open for your feedback.</span></a>
      </div>
      <a class="btn btn--primary" href="#top">Share your feedback</a>
    </div>
  </section>
</main>
<script type="module" src="assets/system.js"></script>
<script type="module" src="assets/motion/motion.js"></script>
<script type="module" src="assets/layout/layout.js"></script>
</body>
</html>
`;

writeFileSync(new URL("./index.html", import.meta.url), html);
console.log("index.html written");
