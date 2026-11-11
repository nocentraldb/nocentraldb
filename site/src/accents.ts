// The candidates for the loud third colour, the owner's choice (60/30/10,
// owner, 2026-11-11). Each renders at /accent/<id>/.
export const accents = [
  { id: "blue", label: "Signal blue", hex: "#2f5bff" },
  { id: "green", label: "Safety green", hex: "#1fd38a" },
  { id: "magenta", label: "Hot magenta", hex: "#ff3d8b" },
] as const;

export type AccentId = (typeof accents)[number]["id"];
