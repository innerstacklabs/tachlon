#!/usr/bin/env node
// The `bin` entry (built to dist-lib/cli/main.js by tsconfig.cli.json): runs the command line and exits with its code. See run.ts.
import { runAsync } from "./run.ts";

const result = await runAsync(process.argv.slice(2), process.cwd());
process.stdout.write(result.out);
process.stderr.write(result.err);
process.exitCode = result.code;
