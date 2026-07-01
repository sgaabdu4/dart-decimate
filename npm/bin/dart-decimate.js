#!/usr/bin/env node

const { runBinary } = require("./runner");

runBinary("dart-decimate", process.argv.slice(2));
