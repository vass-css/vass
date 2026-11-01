"use strict";

const { createHash, randomBytes } = require("node:crypto");
const fs = require("node:fs/promises");
const path = require("node:path");

const packageRoot = path.resolve(__dirname, "..");
const packageJson = require("../package.json");
const allowedHosts = new Set([
  "github.com",
  "release-assets.githubusercontent.com",
  "objects.githubusercontent.com",
]);
const targets = {
  "linux-x64": { triple: "x86_64-unknown-linux-gnu", executable: "vass" },
  "darwin-x64": { triple: "x86_64-apple-darwin", executable: "vass" },
  "darwin-arm64": { triple: "aarch64-apple-darwin", executable: "vass" },
  "win32-x64": { triple: "x86_64-pc-windows-msvc", executable: "vass.exe" },
};
const maximumManifestBytes = 1024 * 1024;
const maximumBinaryBytes = 64 * 1024 * 1024;

function safeHttpsUrl(value) {
  const url = value instanceof URL ? value : new URL(value);
  if (
    url.protocol !== "https:" ||
    !allowedHosts.has(url.hostname) ||
    url.port !== "" ||
    url.username !== "" ||
    url.password !== ""
  ) {
    throw new Error(`Refusing unexpected download URL: ${url.origin}`);
  }
  return url;
}

async function fetchBounded(initialUrl, label, maximumBytes) {
  const controller = new AbortController();
  const timeoutMs = label === "binary" ? 120000 : 30000;
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  let url = safeHttpsUrl(initialUrl);

  try {
    for (let redirects = 0; ; redirects += 1) {
      const response = await fetch(url, {
        redirect: "manual",
        signal: controller.signal,
      });

      if ([301, 302, 303, 307, 308].includes(response.status)) {
        if (redirects >= 5) {
          throw new Error(`Too many redirects while downloading the ${label}`);
        }
        const location = response.headers.get("location");
        await response.body?.cancel();
        if (!location) {
          throw new Error(`Missing redirect location while downloading the ${label}`);
        }
        url = safeHttpsUrl(new URL(location, url));
        continue;
      }

      if (!response.ok) {
        await response.body?.cancel();
        throw new Error(`GitHub returned HTTP ${response.status} for the ${label}`);
      }

      const contentLength = response.headers.get("content-length");
      if (contentLength !== null && Number(contentLength) > maximumBytes) {
        await response.body?.cancel();
        throw new Error(`The ${label} exceeds the ${maximumBytes}-byte download limit`);
      }
      if (!response.body) {
        throw new Error(`GitHub returned an empty response for the ${label}`);
      }

      const reader = response.body.getReader();
      const chunks = [];
      let totalBytes = 0;
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          totalBytes += value.byteLength;
          if (totalBytes > maximumBytes) {
            await reader.cancel();
            throw new Error(`The ${label} exceeds the ${maximumBytes}-byte download limit`);
          }
          chunks.push(Buffer.from(value));
        }
      } finally {
        reader.releaseLock();
      }
      return Buffer.concat(chunks, totalBytes);
    }
  } catch (error) {
    if (controller.signal.aborted) {
      throw new Error(`Timed out downloading the ${label} after ${timeoutMs / 1000} seconds`);
    }
    if (error instanceof TypeError && /fetch|network/i.test(error.message)) {
      throw new Error(`Network error downloading the ${label}: ${error.message}`);
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}

function checksumFor(manifest, assetName) {
  const lines = manifest
    .toString("utf8")
    .split(/\r?\n/)
    .filter((line) => line.length > 0);
  const checksums = new Map();

  for (const line of lines) {
    const match = line.match(/^([a-fA-F0-9]{64})[ \t]+(?:\*)?([^\r\n]+)$/);
    if (!match || match[2].includes("/") || match[2].includes("\\")) {
      throw new Error("The release SHA256SUMS.txt manifest has an invalid entry");
    }
    if (checksums.has(match[2])) {
      throw new Error(`The release manifest contains duplicate checksums for ${match[2]}`);
    }
    checksums.set(match[2], match[1].toLowerCase());
  }

  const checksum = checksums.get(assetName);
  if (!checksum) {
    throw new Error(`The release manifest has no checksum for ${assetName}`);
  }
  return checksum;
}

async function install() {
  if (Number(process.versions.node.split(".")[0]) < 18) {
    throw new Error("Node.js 18 or newer is required to install vass-css");
  }

  const target = targets[`${process.platform}-${process.arch}`];
  if (!target) {
    throw new Error(
      `Unsupported npm platform ${process.platform}-${process.arch}. Supported targets are Linux x64, macOS x64/arm64, and Windows x64.`,
    );
  }

  const version = packageJson.version;
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error("The package version is not a valid safe release version");
  }

  const assetName = `vass-${target.triple}${process.platform === "win32" ? ".exe" : ""}`;
  const releaseBase = `https://github.com/vass-css/vass/releases/download/v${encodeURIComponent(version)}`;
  const manifest = await fetchBounded(
    `${releaseBase}/SHA256SUMS.txt`,
    "SHA256SUMS.txt",
    maximumManifestBytes,
  );
  const expectedChecksum = checksumFor(manifest, assetName);
  const binary = await fetchBounded(
    `${releaseBase}/${encodeURIComponent(assetName)}`,
    assetName,
    maximumBinaryBytes,
  );
  const actualChecksum = createHash("sha256").update(binary).digest("hex");
  if (actualChecksum !== expectedChecksum) {
    throw new Error(
      `SHA-256 verification failed for ${assetName}: expected ${expectedChecksum}, received ${actualChecksum}`,
    );
  }

  const vendorDirectory = path.join(packageRoot, "vendor");
  const binaryPath = path.join(vendorDirectory, target.executable);
  const temporaryPath = path.join(
    vendorDirectory,
    `.vass-${process.pid}-${randomBytes(8).toString("hex")}.tmp`,
  );
  await fs.mkdir(vendorDirectory, { recursive: true });

  try {
    await fs.writeFile(temporaryPath, binary, { flag: "wx", mode: 0o600 });
    if (process.platform !== "win32") {
      await fs.chmod(temporaryPath, 0o755);
    }
    await fs.rename(temporaryPath, binaryPath);
  } finally {
    await fs.rm(temporaryPath, { force: true });
  }
}

install().catch((error) => {
  console.error(`vass-css postinstall failed: ${error.message}`);
  process.exitCode = 1;
});
