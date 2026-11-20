// The landing page's explanatory drawing: a train, a home office and an agent
// each hold the same three tasks, a cloud server sits between them. One pure
// function returns the SVG as a string in four states. Colours and type live in
// site/src/styles/scene.css, on the classes used here.
// Ported from the prototype the owner accepted on 2026-11-12.
// Two layouts: "wide" (800x600, three copies side by side, with their tasks) for
// screens from 640px, and "phone" (400x260), a schematic of the same step without
// any text, for a phone (owner, 2026-11-20, NOC-380).

const W = 230;
const X = { train: 20, home: 285, agent: 550 };
const CARD_Y = 236;
const LINK = {
  train: "M135 236 C135 186 360 200 360 150",
  home: "M400 236 V150",
  agent: "M665 236 C665 186 440 200 440 150",
};
const REVERSE = {
  train: "M360 150 C360 200 135 186 135 236",
  home: "M400 150 V236",
  agent: "M440 150 C440 200 665 186 665 236",
};

// The moment a still drawing shows: 0.72 s, 0.2 of the 3.6 s loop. In each state with
// travelling points one of them is half-way along its line (the pulse windows below),
// and the agent's circling point is visible at any time.
const STILL_AT = 0.72;
const LOOP = 3.6;
// A point frozen at the fraction p of path d: an animateMotion that holds one place.
const frozen = (r, d, p) =>
  `<circle r="${r}" class="sc-pulse"><animateMotion dur="0.001s" fill="freeze" calcMode="linear" keyPoints="${p.toFixed(4)};${p.toFixed(4)}" keyTimes="0;1" path="${d}"/></circle>`;

// A point travelling along path d between the fractions t0 and t1 of a 3.6 s loop.
// Still, it stands where the moving one is at STILL_AT, or is left out when that one
// is hidden then.
const pulse = (d, t0, t1, still) => {
  if (still) {
    const f = STILL_AT / LOOP;
    if (f < t0 + 0.01 || f > t1 - 0.01) return "";
    return frozen(6, d, (f - t0) / (t1 - t0));
  }
  return `
  <circle r="6" class="sc-pulse" opacity="0">
    <animateMotion dur="3.6s" repeatCount="indefinite" calcMode="linear" keyPoints="0;0;1;1" keyTimes="0;${t0};${t1};1" path="${d}"/>
    <animate attributeName="opacity" dur="3.6s" repeatCount="indefinite" values="0;0;1;1;0;0" keyTimes="0;${t0};${(t0 + 0.01).toFixed(2)};${(t1 - 0.01).toFixed(2)};${t1};1"/>
  </circle>`;
};
// The agent's reads: a point circling its card's edge once every 1.4 s.
const circling = (perimeter, still) =>
  still
    ? frozen(5, perimeter, (STILL_AT % 1.4) / 1.4)
    : `<circle r="5" class="sc-pulse"><animateMotion dur="1.4s" repeatCount="indefinite" path="${perimeter}"/></circle>`;

// One copy of the task list. rows: [title, status, kind]; kind is "origin" (changed here) or "got" (arrived here).
const card = (
  who,
  label,
  rows,
  { offline = false, loop = false, still = false } = {},
) => {
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
    ${loop ? circling(perimeter, still) : ""}
  </g>`;
};

// The cloud server with two rows of ciphertext; with the lens, a magnified view of it.
const cloud = (lens, id) => `
  <path d="M312 150 H490 a44 44 0 0 0 6 -87 a58 58 0 0 0 -108 -20 a50 50 0 0 0 -92 26 a41 41 0 0 0 16 81 Z" class="sc-cloud"/>
  <text x="400" y="92" text-anchor="middle" class="sc-label">Cloud server</text>
  <text x="400" y="116" text-anchor="middle" class="sc-cipher">9f3a c01e 77b2 d54a</text>
  <text x="400" y="134" text-anchor="middle" class="sc-cipher">0c6e 1f8b 3d92 a7e4</text>
  ${
    lens
      ? `
  <line x1="573" y1="137" x2="604" y2="168" class="sc-handle"/>
  <circle cx="528" cy="92" r="64" class="sc-lens"/>
  <clipPath id="${id}-lens-clip"><circle cx="528" cy="92" r="60"/></clipPath>
  <g clip-path="url(#${id}-lens-clip)">
    <text x="528" y="68" text-anchor="middle" class="sc-cipher-big">e77b2d54</text>
    <text x="528" y="100" text-anchor="middle" class="sc-cipher-big">a0c6e1f8</text>
    <text x="528" y="132" text-anchor="middle" class="sc-cipher-big">b3d92a7e</text>
  </g>`
      : ""
  }`;

// The train at x offset tx; clipX hides everything right of it (inside the tunnel).
const train = (tx, clipX, id) => `
  <g ${clipX ? `clip-path="url(#${id}-train-clip-${clipX})"` : ""}>
    ${clipX ? `<clipPath id="${id}-train-clip-${clipX}"><rect x="0" y="440" width="${clipX}" height="160"/></clipPath>` : ""}
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

// The three places under the copies. tunnel: undefined (before), "in" or "out".
const trainPlace = (tunnel, id) => `
  <line x1="0" y1="566" x2="322" y2="566" class="sc-track"/>
  ${hill(tunnel === "out" ? "behind" : "ahead")}
  ${tunnel === "in" ? train(150, 221, id) : tunnel === "out" ? train(150, 0, id) : train(20, 0, id)}`;
const housePlace = () => `
  <path d="M340 566 V508 L405 464 L470 508 V566 Z" class="sc-thing"/>
  <rect x="389" y="518" width="32" height="28" rx="5" class="sc-window sc-window-lit"/>`;
const rackPlace = () => `
  <rect x="595" y="482" width="140" height="84" rx="12" class="sc-thing"/>
  <line x1="595" y1="510" x2="735" y2="510" class="sc-track"/><line x1="595" y1="538" x2="735" y2="538" class="sc-track"/>
  <circle cx="614" cy="496" r="4" class="sc-lamp"/><circle cx="614" cy="524" r="4" class="sc-lamp"/><circle cx="614" cy="552" r="4" class="sc-lamp"/>`;

const link = (who, cut) =>
  `<path d="${LINK[who]}" class="sc-link ${cut ? "sc-link-cut" : ""}"/>`;
const links = (cutTrain) =>
  `${link("train", cutTrain)}${link("home")}${link("agent")}`;

const R = {
  readme: (s, k) => ["Write README", s, k],
  rfc: (s, k) => ["Review RFC", s, k],
  release: (s, k) => ["Plan release", s, k],
};
const open3 = [R.readme("open"), R.rfc("open"), R.release("open")];

// The four states as data: what each one shows, apart from how a layout draws it.
// pulses: [who, "up" (copy to cloud) | "down" (cloud to copy), from, to] in fractions of the loop.
const STATES = [
  {
    cards: { train: open3, home: open3, agent: open3 },
    loop: true,
    pulses: [],
  },
  {
    lens: true,
    cards: {
      train: [
        R.readme("in progress", "origin"),
        R.rfc("open"),
        R.release("open"),
      ],
      home: [R.readme("in progress", "got"), R.rfc("open"), R.release("open")],
      agent: [R.readme("in progress", "got"), R.rfc("open"), R.release("open")],
    },
    pulses: [
      ["train", "up", 0.05, 0.4],
      ["home", "down", 0.45, 0.8],
      ["agent", "down", 0.45, 0.8],
    ],
  },
  {
    offline: true,
    tunnel: "in",
    loop: true,
    cards: {
      train: [
        R.readme("in progress"),
        R.rfc("done · stored locally", "origin"),
        R.release("open"),
      ],
      home: [
        R.readme("in progress"),
        R.rfc("open"),
        R.release("assigned to agent", "origin"),
      ],
      agent: [
        R.readme("in progress"),
        R.rfc("open"),
        R.release("assigned to agent", "got"),
      ],
    },
    pulses: [
      ["home", "up", 0.05, 0.3],
      ["agent", "down", 0.32, 0.57],
      ["agent", "up", 0.6, 0.8],
      ["home", "down", 0.82, 0.98],
    ],
  },
  {
    tunnel: "out",
    cards: {
      train: [
        R.readme("in progress"),
        R.rfc("done"),
        R.release("assigned to agent", "got"),
      ],
      home: [
        R.readme("in progress"),
        R.rfc("done", "got"),
        R.release("assigned to agent"),
      ],
      agent: [
        R.readme("in progress"),
        R.rfc("done", "got"),
        R.release("assigned to agent"),
      ],
    },
    pulses: [
      ["train", "up", 0.05, 0.3],
      ["home", "down", 0.32, 0.57],
      ["agent", "down", 0.32, 0.57],
      ["train", "down", 0.6, 0.9],
    ],
  },
];
const LABEL = { train: "On the train", home: "Home office", agent: "Agent" };
const WHO = ["train", "home", "agent"];

const wide = (st, id, still) => `${links(st.offline)}${cloud(st.lens, id)}
  ${st.pulses.map(([who, dir, t0, t1]) => pulse((dir === "up" ? LINK : REVERSE)[who], t0, t1, still)).join("")}
  ${card("train", LABEL.train, st.cards.train, { offline: st.offline })}
  ${card("home", LABEL.home, st.cards.home)}
  ${card("agent", LABEL.agent, st.cards.agent, { loop: st.loop, still })}
  ${trainPlace(st.tunnel, id)}${housePlace()}${rackPlace()}`;

// The phone layout (owner, 2026-11-20, NOC-380): one small schematic per step and no
// text at all, since the step's title and text carry the words. The three places as
// icons with their copy under them (a card of three bars, one per task), the server as
// a cloud, and one cue for what the step does: a change marked on a bar and its way
// drawn as an arrow, the lock on the cloud, the train in its tunnel with its link cut.
const PX = { train: 70, home: 200, agent: 330 }; // the places' centres
const GROUND = 200;
const SMALL = 0.38;
// [the place's centre in the wide drawing, how it is drawn]
const ICON = {
  train: [161, (st, id) => trainPlace(st.tunnel, id)],
  home: [405, () => housePlace()],
  agent: [665, () => rackPlace()],
};
const icon = (who, st, id) => {
  const [centre, draw] = ICON[who];
  return `<g transform="translate(${(PX[who] - centre * SMALL).toFixed(2)} ${(GROUND - 566 * SMALL).toFixed(2)}) scale(${SMALL})">${draw(st, id)}</g>`;
};
const UP = {
  train: "M70 152 C70 118 168 122 176 86",
  home: "M200 152 V86",
  agent: "M330 152 C330 118 232 122 224 86",
};
const DOWN = {
  train: "M176 86 C168 122 70 118 70 152",
  home: "M200 86 V152",
  agent: "M224 86 C232 122 330 118 330 152",
};
// The marks on each copy's three bars, and the flows: [who, "up" | "down" | "both"].
const SKETCH = [
  {
    marks: { train: ["", "", ""], home: ["", "", ""], agent: ["", "", ""] },
    flows: [],
  },
  {
    lock: true,
    marks: {
      train: ["origin", "", ""],
      home: ["got", "", ""],
      agent: ["got", "", ""],
    },
    flows: [
      ["train", "up"],
      ["home", "down"],
      ["agent", "down"],
    ],
  },
  {
    offline: true,
    tunnel: "in",
    marks: {
      train: ["", "origin", ""],
      home: ["", "", "origin"],
      agent: ["", "", "got"],
    },
    flows: [
      ["home", "up"],
      ["agent", "down"],
    ],
  },
  {
    tunnel: "out",
    marks: {
      train: ["", "got", "got"],
      home: ["", "got", "got"],
      agent: ["", "got", "got"],
    },
    flows: [["train", "both"]],
  },
];
const copy = (who, marks, offline) => {
  const x = PX[who];
  return `
  <rect x="${x - 32}" y="210" width="64" height="46" rx="8" class="sc-card"/>
  ${marks.map((mark, i) => `<rect x="${x - 26}" y="${216 + i * 14}" width="52" height="10" rx="3" class="sc-row${mark ? ` sc-row-${mark}` : ""}"/>`).join("")}
  <circle cx="${x + 42}" cy="214" r="5" class="${offline ? "sc-dot-off" : "sc-dot-on"}"/>`;
};
const flow = ([who, dir], id) => {
  const d = (dir === "down" ? DOWN : UP)[who];
  const start = dir === "both" ? ` marker-start="url(#${id}-arrow)"` : "";
  return `<path d="${d}" class="sc-flow"${start} marker-end="url(#${id}-arrow)"/>`;
};
const lock = () => `
  <path d="M191 50 V44 a9 9 0 0 1 18 0 V50" class="sc-lock"/>
  <rect x="186" y="50" width="28" height="22" rx="4" class="sc-lock-body"/>`;
const small = (step, id) => {
  const st = SKETCH[step];
  return `
  <defs><marker id="${id}-arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="4" markerHeight="4" orient="auto-start-reverse"><path d="M0 0 L10 5 L0 10 Z" class="sc-arrow"/></marker></defs>
  ${WHO.map((who) => `<path d="${UP[who]}" class="sc-link${who === "train" && st.offline ? " sc-link-cut" : ""}"/>`).join("")}
  <path transform="translate(-48 -7) scale(0.62)" d="M312 150 H490 a44 44 0 0 0 6 -87 a58 58 0 0 0 -108 -20 a50 50 0 0 0 -92 26 a41 41 0 0 0 16 81 Z" class="sc-cloud"/>
  ${st.lock ? lock() : ""}
  ${st.flows.map((f) => flow(f, id)).join("")}
  ${WHO.map((who) => icon(who, st, id)).join("")}
  ${WHO.map((who) => copy(who, st.marks[who], who === "train" && st.offline)).join("")}`;
};

// title and text for the step, alt for the drawing's aria-label.
export const STEPS = [
  {
    title: "Everyone has the full task list",
    text: "The traveller, the person in the home office and the agent in the cloud each keep the whole task list on their own disk. Everyone reads locally; reading needs no network connection.",
    alt: "The traveller on a train, a person in a home office and an agent each hold the same three tasks. A server in the cloud sits between them.",
  },
  {
    title: "A change travels encrypted",
    text: "The traveller claims a task. The change goes through the server to the other two. The server sees only encrypted bytes and has no key to read them.",
    alt: "The traveller claims a task. The change travels encrypted through the server to the home office and the agent. A lens on the server shows only encrypted bytes.",
  },
  {
    title: "The traveller goes offline",
    text: "The train enters a tunnel, but the traveller keeps working; the changes are stored locally. The other two carry on from the last state they know and don't see the traveller's changes yet.",
    alt: "The traveller's train is in a tunnel and offline. The change is stored locally. The home office and the agent keep exchanging changes.",
  },
  {
    title: "The traveller reconnects",
    text: "The traveller's changes go out and the others' changes come in. All three copies end up in the same state.",
    alt: "The traveller is back online. Changes cross in both directions, and all three copies show the same tasks.",
  },
];

// The drawing for step 0..3 as an SVG string, in the "wide" or the "phone" layout.
// idPrefix starts every id inside it, so several drawings can share one page; the
// phone layout adds "-phone", so a wide and a phone drawing may share a prefix.
// still: the wide picture at STILL_AT, with nothing that moves (the scenario,
// NOC-380); the phone layout never moves.
export function scene(step, idPrefix, layout = "wide", { still = false } = {}) {
  const [box, body] =
    layout === "phone"
      ? ["0 0 400 260", small(step, `${idPrefix}-phone`)]
      : ["0 0 800 600", wide(STATES[step], idPrefix, still)];
  return `<svg class="sc sc--${layout}" viewBox="${box}" role="img" aria-label="${STEPS[step].alt}">${body}</svg>`;
}
