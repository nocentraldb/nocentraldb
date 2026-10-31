// Minimal glob matching for the allowlist of check-fragments: `**/` any
// directory prefix, `**` anything, `*` within one segment, `?` one character.
const escapeRegExp = (s) => s.replace(/[.+^${}()|[\]\\]/g, "\\$&");
export function globToRegExp(pattern) {
  let re = "";
  for (let i = 0; i < pattern.length; i++) {
    if (pattern.startsWith("**/", i)) {
      re += "(?:.*/)?";
      i += 2;
    } else if (pattern.startsWith("**", i)) {
      re += ".*";
      i += 1;
    } else if (pattern[i] === "*") re += "[^/]*";
    else if (pattern[i] === "?") re += "[^/]";
    else re += escapeRegExp(pattern[i]);
  }
  return new RegExp(`^${re}$`);
}
export const matchesAny = (path, patterns) =>
  patterns.some((p) => globToRegExp(p).test(path));
