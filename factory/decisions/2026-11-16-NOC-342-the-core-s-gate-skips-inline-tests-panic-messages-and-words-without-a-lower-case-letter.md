# 2026-11-16 · NOC-342 · The core's gate skips inline tests, panic messages and words without a lower-case letter

- Decided by: pm, NOC-342, on the coding agent's proposal: the core's gate skips inline tests, panic messages and words without a lower-case letter
- Why: pm decided that the gate reads `crates/ncdb-core/src` with T5's rule: any literal with two words in a row is a text written in code, except on a line marked `// notation`. Over all of the core, that rule flagged 210 places. Only about 90 were texts. The rest were 49 SQL statements and fragments (`"SELECT frame FROM log WHERE …"`, `"({name} IS NOT NULL OR …)"`), 45 panic messages (`.expect("the log holds the change")`, `unreachable!("a genesis node")`), and 4 lines in inline `#[cfg(test)]` modules. For the core, three refinements fix this:
  - **An inline `#[cfg(test)]` module is a test**, as a test file is.
  - **A panic's message is no text.** That covers `expect`, `unreachable!`, `panic!`, `assert!`, `assert_eq!` and `assert_ne!`. Such a message describes a bug in the core, not something a developer acts on.
  - **A word has a lower-case letter.** So SQL's keywords in capitals are no words, and `FROM log WHERE` never holds two words in a row. The rule errs on the safe side: lower-case SQL would still be flagged, and a text in all capitals is not one we would write.

  The gate also reads a literal continued over lines with a backslash as one literal. That finds one more text: the lock's unknown format. With these rules, the gate flags exactly the texts: 93 places, including 2 lines of notation, which are marked (`"ncdb/set of {}"`, a set's type as a person reads it, and the plugin registry's Debug label).
- Instead of: about 50 lines of SQL marked `// notation`, and 45 more for the panics or 45 panic messages in texts.json; or T5's rule unchanged, which would leave the gate a list of exceptions.
- Topics: texts, core, gates
