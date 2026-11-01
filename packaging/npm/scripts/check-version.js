"use strict";

const fs = require("node:fs");
const path = require("node:path");

const packageRoot = path.resolve(__dirname, "..");
const packageJson = JSON.parse(fs.readFileSync(path.join(packageRoot, "package.json"), "utf8"));
const cargoToml = fs.readFileSync(path.resolve(packageRoot, "../../Cargo.toml"), "utf8");
const workspaceVersion = cargoToml.match(
  /^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
);

if (!workspaceVersion) {
  throw new Error("Could not read the workspace package version from Cargo.toml");
}

if (packageJson.version !== workspaceVersion[1]) {
  throw new Error(
    `npm package version ${packageJson.version} does not match Cargo workspace version ${workspaceVersion[1]}`,
  );
}

const tag = process.argv[2];
if (tag !== undefined) {
  const tagVersion = tag.startsWith("v") ? tag.slice(1) : "";
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(tagVersion)) {
    throw new Error(`Release tag must be a version tag such as v${packageJson.version}`);
  }
  if (tagVersion !== packageJson.version) {
    throw new Error(
      `Release tag ${tag} does not match npm and Cargo version ${packageJson.version}`,
    );
  }
}

console.log(`Validated package version ${packageJson.version}`);
