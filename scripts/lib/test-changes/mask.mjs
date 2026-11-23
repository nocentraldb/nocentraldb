// Blanks what is not code for the test-change check (NOC-508), so that a
// test, a marker or an assertion inside a string or a comment does not count
// and a bracket inside one does not end an assertion early.
//
// mask(text, kind) returns two texts of the same length and lines as `text`:
// - `code`: comments and the insides of string, char, template and regular
//   expression literals blanked to spaces; the delimiters stay.
// - `plain`: only the comments blanked, so a literal's text can be read at
//   the indexes found in `code`.
// It is a lexer for these purposes only, not a parser: a regular expression
// is told from a division by the character before it.

const blank = (s) => s.replace(/[^\n]/g, " ");

/** The index just past the quote that closes the literal opened at `i`. */
function pastQuoted(text, i, quote) {
  for (let j = i + 1; j < text.length; j += 1) {
    if (text[j] === "\\") j += 1;
    else if (text[j] === quote) return j + 1;
  }
  return text.length;
}

/** The index just past a TypeScript template literal, `${…}` included. */
function pastTemplate(text, i) {
  for (let j = i + 1; j < text.length; j += 1) {
    if (text[j] === "\\") j += 1;
    else if (text[j] === "`") return j + 1;
    else if (text[j] === "$" && text[j + 1] === "{") j = pastCode(text, j + 2) - 1;
  }
  return text.length;
}

/** The index just past the `}` that closes the code starting at `i`. */
function pastCode(text, i) {
  let depth = 0;
  for (let j = i; j < text.length; j += 1) {
    const c = text[j];
    if (c === '"' || c === "'") j = pastQuoted(text, j, c) - 1;
    else if (c === "`") j = pastTemplate(text, j) - 1;
    else if (c === "{") depth += 1;
    else if (c === "}" && (depth -= 1) < 0) return j + 1;
  }
  return text.length;
}

const REGEX_BEFORE =
  /(?:[(,=:[!&|?{};+\-*%<>~^]|\b(?:return|typeof|case|in|of|void|delete|new|throw|yield|await|else|do))\s*$/;

/** Whether a `/` after `code` starts a regular expression. */
const regexAfter = (code) =>
  !/\S/.test(code) || REGEX_BEFORE.test(code.slice(-16));

/** The index just past a regular expression literal at `i`, or -1. */
function pastRegex(text, i) {
  let inClass = false;
  for (let j = i + 1; j < text.length; j += 1) {
    const c = text[j];
    if (c === "\n") return -1;
    if (c === "\\") j += 1;
    else if (c === "[") inClass = true;
    else if (c === "]") inClass = false;
    else if (c === "/" && !inClass) {
      let k = j + 1;
      while (/[a-z]/i.test(text[k] ?? "")) k += 1;
      return k;
    }
  }
  return -1;
}

/** The index just past a Rust block comment at `i`; they nest. */
function pastBlockComment(text, i, nests) {
  let depth = 0;
  for (let j = i; j < text.length; j += 1) {
    if (text.startsWith("/*", j) && (nests || depth === 0)) {
      depth += 1;
      j += 1;
    } else if (text.startsWith("*/", j)) {
      depth -= 1;
      j += 1;
      if (depth === 0) return j + 1;
    }
  }
  return text.length;
}

/** A Rust string at `i` (`"`, `r"`, `r#"`, `b"`, `br#"`, `c"`): its end and its quotes. */
function rustString(text, i) {
  const m = /^(?:b|c)?(r(#*))?"/.exec(text.slice(i, i + 64));
  if (!m || (i > 0 && /[\w$]/.test(text[i - 1]) && m[0] !== '"')) return null;
  const open = m[0].length;
  if (m[1] === undefined) {
    return { end: pastQuoted(text, i + open - 1, '"'), open, close: 1 };
  }
  const closing = `"${m[2]}`;
  const at = text.indexOf(closing, i + open);
  const end = at === -1 ? text.length : at + closing.length;
  return { end, open, close: closing.length };
}

/** A Rust char literal at `i` (`'a'`, `'\n'`), not a lifetime; its end. */
function rustChar(text, i) {
  if (text[i + 1] === "\\") {
    const close = text.indexOf("'", i + 3);
    return close === -1 ? null : close + 1;
  }
  const width = (text.codePointAt(i + 1) ?? 0) > 0xffff ? 2 : 1;
  return text[i + 1 + width] === "'" ? i + 2 + width : null;
}

export function mask(text, kind) {
  let code = "";
  let plain = "";
  let i = 0;
  const literal = (end, open, close) => {
    const inner = text.slice(i + open, end - close);
    code += text.slice(i, i + open) + blank(inner) + text.slice(end - close, end);
    plain += text.slice(i, end);
    i = end;
  };
  const comment = (end) => {
    code += blank(text.slice(i, end));
    plain += blank(text.slice(i, end));
    i = end;
  };
  while (i < text.length) {
    const c = text[i];
    if (text.startsWith("//", i)) {
      const nl = text.indexOf("\n", i);
      comment(nl === -1 ? text.length : nl);
    } else if (text.startsWith("/*", i)) {
      comment(pastBlockComment(text, i, kind !== "ts"));
    } else if (kind === "ts" && (c === '"' || c === "'")) {
      literal(pastQuoted(text, i, c), 1, 1);
    } else if (kind === "ts" && c === "`") {
      literal(pastTemplate(text, i), 1, 1);
    } else if (kind === "ts" && c === "/" && regexAfter(code)) {
      const end = pastRegex(text, i);
      if (end === -1) {
        code += c;
        plain += c;
        i += 1;
      } else {
        const close = text.lastIndexOf("/", end - 1);
        literal(end, 1, end - close);
      }
    } else if (kind !== "ts" && /[brc"]/.test(c) && rustString(text, i)) {
      const { end, open, close } = rustString(text, i);
      literal(end, open, close);
    } else if (kind !== "ts" && c === "'" && rustChar(text, i)) {
      literal(rustChar(text, i), 1, 1);
    } else {
      code += c;
      plain += c;
      i += 1;
    }
  }
  return { code, plain };
}
