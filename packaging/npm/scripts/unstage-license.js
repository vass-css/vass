"use strict";

const fs = require("node:fs");
const path = require("node:path");

fs.rmSync(path.resolve(__dirname, "../LICENSE"), { force: true });
