"use strict";

const fs = require("node:fs");
const path = require("node:path");

fs.copyFileSync(path.resolve(__dirname, "../../../LICENSE"), path.resolve(__dirname, "../LICENSE"));
