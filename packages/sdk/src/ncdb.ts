#!/usr/bin/env node
// `npx ncdb`: the bin package.json names (NOC-415). Its core sits in
// dist/ncdb/, next to this file once built.
import { start } from "./ncdbCli.ts";

await start(new URL("./ncdb/", import.meta.url));
