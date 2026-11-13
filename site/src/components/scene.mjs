// The landing page's explanatory drawing: a train, a home office and an agent
// each hold the same three tasks, a cloud server sits between them. One pure
// function returns the SVG as a string in four states. Colours and type live in
// site/src/styles/scene.css, on the classes used here.
// Ported from the prototype the owner accepted on 2026-11-12.
// Two layouts draw the same states: "wide" (800x600, three copies side by side) for
// screens from 640px, and "tall" (400x704, the copies stacked, one line per task)
// for a phone, where the wide drawing's labels would be too small to read.

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

// A point travelling along path d between the fractions t0 and t1 of a 3.6 s loop.
const pulse = (d, t0, t1) => `
  <circle r="6" class="sc-pulse" opacity="0">
    <animateMotion dur="3.6s" repeatCount="indefinite" calcMode="linear" keyPoints="0;0;1;1" keyTimes="0;${t0};${t1};1" path="${d}"/>
    <animate attributeName="opacity" dur="3.6s" repeatCount="indefinite" values="0;0;1;1;0;0" keyTimes="0;${t0};${(t0 + 0.01).toFixed(2)};${(t1 - 0.01).toFixed(2)};${t1};1"/>
  </circle>`;

// One copy of the task list. rows: [title, status, kind]; kind is "origin" (changed here) or "got" (arrived here).
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
    ${loop ? `<circle r="5" class="sc-pulse"><animateMotion dur="1.4s" repeatCount="indefinite" path="${perimeter}"/></circle>` : ""}
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

const wide = (st, id) => `${links(st.offline)}${cloud(st.lens, id)}
  ${st.pulses.map(([who, dir, t0, t1]) => pulse((dir === "up" ? LINK : REVERSE)[who], t0, t1)).join("")}
  ${card("train", LABEL.train, st.cards.train, { offline: st.offline })}
  ${card("home", LABEL.home, st.cards.home)}
  ${card("agent", LABEL.agent, st.cards.agent, { loop: st.loop })}
  ${trainPlace(st.tunnel, id)}${housePlace()}${rackPlace()}`;

// The tall layout: the cloud on top, the three copies stacked under it, each with its
// place drawn small in its header and its own line up the left edge to the cloud.
const T = { x: 50, w: 336, top: 176, h: 164, gap: 12, head: 56, foot: 138 };
const LANE = { train: [36, 160], home: [25, 142], agent: [14, 124] }; // [lane x, x at the cloud's foot]
const tallY = (who) => T.top + WHO.indexOf(who) * (T.h + T.gap);
const tallUp = (who) => {
  const [lx, cx] = LANE[who];
  const by = tallY(who) + 28;
  return `M${T.x} ${by} H${lx + 10} Q${lx} ${by} ${lx} ${by - 10} V170 C${lx} 150 ${cx} 162 ${cx} ${T.foot}`;
};
const tallDown = (who) => {
  const [lx, cx] = LANE[who];
  const by = tallY(who) + 28;
  return `M${cx} ${T.foot} C${cx} 162 ${lx} 150 ${lx} 170 V${by - 10} Q${lx} ${by} ${lx + 10} ${by} H${T.x}`;
};
// A place at 0.45 of its size, its right edge at `right` and its ground line at `ground`.
const PLACE = {
  train: [322, trainPlace],
  home: [470, housePlace],
  agent: [735, rackPlace],
};
const tallPlace = (who, tunnel, id, right, ground) => {
  const [x1, draw] = PLACE[who];
  return `<g transform="translate(${right - x1 * 0.45} ${ground - 566 * 0.45}) scale(0.45)">${draw(tunnel, id)}</g>`;
};
const tallCard = (who, rows, st, id) => {
  const { x, w, h, head } = T;
  const y = tallY(who);
  const offline = who === "train" && st.offline;
  const perimeter = `M${x + 14} ${y} H${x + w - 14} a14 14 0 0 1 14 14 V${y + h - 14} a14 14 0 0 1 -14 14 H${x + 14} a14 14 0 0 1 -14 -14 V${y + 14} a14 14 0 0 1 14 -14 Z`;
  return `
  <g>
    <rect x="${x}" y="${y}" width="${w}" height="${h}" rx="14" class="sc-card"/>
    <text x="${x + 16}" y="${y + 35}" class="sc-label">${LABEL[who]}</text>
    ${tallPlace(who, st.tunnel, id, x + w - 40, y + head - 6)}
    <circle cx="${x + w - 20}" cy="${y + 28}" r="5" class="${offline ? "sc-dot-off" : "sc-dot-on"}"/>
    ${rows
      .map(([title, status, kind], i) => {
        const ry = y + head + i * 34;
        const on = kind === "origin" ? " sc-on-accent" : "";
        return `
    <rect x="${x + 8}" y="${ry}" width="${w - 16}" height="30" rx="8" class="sc-row ${kind ? `sc-row-${kind}` : ""}"/>
    <text x="${x + 20}" y="${ry + 21}" class="sc-title${on}">${title}</text>
    <text x="${x + w - 20}" y="${ry + 21}" text-anchor="end" class="sc-status${on}">${status}</text>`;
      })
      .join("")}
    ${who === "agent" && st.loop ? `<circle r="5" class="sc-pulse"><animateMotion dur="1.4s" repeatCount="indefinite" path="${perimeter}"/></circle>` : ""}
  </g>`;
};
const tallCloud = (lens, id) => `
  <path transform="translate(-190 -12)" d="M312 150 H490 a44 44 0 0 0 6 -87 a58 58 0 0 0 -108 -20 a50 50 0 0 0 -92 26 a41 41 0 0 0 16 81 Z" class="sc-cloud"/>
  <text x="210" y="80" text-anchor="middle" class="sc-label">Cloud server</text>
  <text x="210" y="104" text-anchor="middle" class="sc-cipher">9f3a c01e 77b2</text>
  <text x="210" y="124" text-anchor="middle" class="sc-cipher">0c6e 1f8b 3d92</text>
  ${
    lens
      ? `
  <line x1="361" y1="109" x2="384" y2="132" class="sc-handle"/>
  <circle cx="322" cy="70" r="54" class="sc-lens"/>
  <clipPath id="${id}-lens-clip"><circle cx="322" cy="70" r="50"/></clipPath>
  <g clip-path="url(#${id}-lens-clip)">
    <text x="322" y="50" text-anchor="middle" class="sc-cipher-big">e77b2d54</text>
    <text x="322" y="79" text-anchor="middle" class="sc-cipher-big">a0c6e1f8</text>
    <text x="322" y="108" text-anchor="middle" class="sc-cipher-big">b3d92a7e</text>
  </g>`
      : ""
  }`;
const tall = (st, id) => `
  ${WHO.map((who) => `<path d="${tallUp(who)}" class="sc-link ${who === "train" && st.offline ? "sc-link-cut" : ""}"/>`).join("")}
  ${tallCloud(st.lens, id)}
  ${st.pulses.map(([who, dir, t0, t1]) => pulse((dir === "up" ? tallUp : tallDown)(who), t0, t1)).join("")}
  ${WHO.map((who) => tallCard(who, st.cards[who], st, id)).join("")}`;

// title and text for the step, alt for the drawing's aria-label.
export const STEPS = [
  {
    title: "Everyone holds a full copy",
    text: "The laptop on the train, the desk at home and the agent each keep the whole task list on their own disk. The agent reads its copy as often as it likes, and no read crosses the network.",
    alt: "A laptop on a train, a desk in a home office and an agent each hold the same three tasks. A cloud server sits between them.",
  },
  {
    title: "A change travels as ciphertext",
    text: "The train marks a task as in progress. The change goes through a server in the cloud to the other two. Hold a lens over that server and you see bytes it has no key for.",
    alt: "The train changes one task. The change travels through the cloud server to the home office and the agent. A lens on the server shows only ciphertext.",
  },
  {
    title: "The train enters a tunnel",
    text: "The laptop is offline and its owner keeps working; the change is stored locally. At home and with the agent the work goes on too. They don't see the train's change yet.",
    alt: "The train is in a tunnel and offline. Its change is stored locally. The home office and the agent keep exchanging changes.",
  },
  {
    title: "Out of the tunnel, every copy matches",
    text: "The laptop reconnects. Its change goes out, the others' changes come in, and all three copies end in the same state.",
    alt: "The train has left the tunnel. Changes cross in both directions and all three copies show the same tasks.",
  },
];

// The drawing for step 0..3 as an SVG string, in the "wide" or the "tall" layout.
// idPrefix starts every id inside it, so several drawings can share one page; the
// tall layout adds "-tall", so a wide and a tall drawing may share a prefix.
export function scene(step, idPrefix, layout = "wide") {
  const [box, body] =
    layout === "tall"
      ? ["0 0 400 704", tall(STATES[step], `${idPrefix}-tall`)]
      : ["0 0 800 600", wide(STATES[step], idPrefix)];
  return `<svg class="sc sc--${layout}" viewBox="${box}" role="img" aria-label="${STEPS[step].alt}">${body}</svg>`;
}
