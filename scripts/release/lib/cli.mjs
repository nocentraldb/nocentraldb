// The command-line conventions every release script shares, in the style of
// scripts/check-pins.mjs: `fail(message)` prints `<name>: <message>` on stderr
// and sets exit code 1 without stopping, so a script reports every violation;
// `usageError()` prints the usage on stderr and exits 2; `parseArgs` reads
// `--flag` switches and `--key <value>` options, prints the usage and exits 0
// on --help, and treats anything else (an unknown option, a positional, an
// option without its value) as a usage error.

const camel = (option) =>
  option.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());

export function cli(name, usage) {
  const fail = (message) => {
    console.error(`${name}: ${message}`);
    process.exitCode = 1;
  };
  const usageError = () => {
    console.error(usage);
    process.exit(2);
  };
  /** { flags: [...], values: [...], defaults: {} } → the options, camel-cased. */
  const parseArgs = (args, { flags = [], values = [], defaults = {} } = {}) => {
    const opts = { ...defaults };
    for (let i = 0; i < args.length; i++) {
      const arg = args[i];
      if (arg === "--help") {
        console.log(usage);
        process.exit(0);
      } else if (flags.includes(arg)) opts[camel(arg)] = true;
      else if (values.includes(arg) && args[i + 1])
        opts[camel(arg)] = args[++i];
      else usageError();
    }
    return opts;
  };
  return { fail, usageError, parseArgs };
}
