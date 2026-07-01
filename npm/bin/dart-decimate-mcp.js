#!/usr/bin/env node

const { runBinary } = require("./runner");

runBinary("dart-decimate-mcp", process.argv.slice(2));
