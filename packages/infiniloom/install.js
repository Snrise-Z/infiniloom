#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const https = require("https");
const { execSync } = require("child_process");

const VERSION = require("./package.json").version;
const REPO = "Topos-Labs/infiniloom";

// Map Node.js platform/arch to release artifact names
function getArtifactInfo() {
  const platform = process.platform;
  const arch = process.arch;

  // Artifact names must match the release workflow output (release.yml)
  const mapping = {
    "darwin-x64": { artifact: "infiniloom-darwin-x64.tar.gz", binary: "infiniloom" },
    "darwin-arm64": { artifact: "infiniloom-darwin-arm64.tar.gz", binary: "infiniloom" },
    "linux-x64": { artifact: "infiniloom-linux-x64.tar.gz", binary: "infiniloom" },
    "linux-arm64": { artifact: "infiniloom-linux-arm64.tar.gz", binary: "infiniloom" },
  };

  const key = `${platform}-${arch}`;
  const info = mapping[key];

  if (!info) {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    console.error("Supported: darwin-x64, darwin-arm64, linux-x64, linux-arm64");
    console.error("");
    console.error("Install from source instead:");
    console.error("  cargo install infiniloom");
    process.exit(1);
  }

  return info;
}

function downloadFile(url, destPath) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destPath);
    const request = (url) => {
      https.get(url, { headers: { "User-Agent": "infiniloom-npm" } }, (res) => {
        if (res.statusCode === 302 || res.statusCode === 301) {
          request(res.headers.location);
          return;
        }
        if (res.statusCode !== 200) {
          fs.unlinkSync(destPath);
          reject(new Error(`HTTP ${res.statusCode}: ${res.statusMessage}`));
          return;
        }
        res.pipe(file);
        file.on("finish", () => {
          file.close();
          resolve();
        });
      }).on("error", (err) => {
        fs.unlinkSync(destPath);
        reject(err);
      });
    };
    request(url);
  });
}

async function install() {
  const binDir = path.join(__dirname, "bin");
  const { artifact, binary } = getArtifactInfo();
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${artifact}`;
  const tmpDir = path.join(__dirname, ".tmp");
  const archivePath = path.join(tmpDir, artifact);

  // Create directories
  if (!fs.existsSync(binDir)) fs.mkdirSync(binDir, { recursive: true });
  if (!fs.existsSync(tmpDir)) fs.mkdirSync(tmpDir, { recursive: true });

  console.log(`Downloading infiniloom v${VERSION}...`);

  try {
    await downloadFile(url, archivePath);

    // Extract archive
    execSync(`tar -xzf "${archivePath}" -C "${tmpDir}"`, { stdio: "pipe" });

    // Find and move binary
    const srcPath = path.join(tmpDir, binary);
    const destPath = path.join(binDir, "infiniloom-bin");

    if (!fs.existsSync(srcPath)) {
      throw new Error(`Binary not found in archive: ${binary}`);
    }

    fs.copyFileSync(srcPath, destPath);
    fs.chmodSync(destPath, 0o755);

    // Clean up
    fs.rmSync(tmpDir, { recursive: true, force: true });

    console.log(`Infiniloom v${VERSION} installed successfully!`);
  } catch (error) {
    // Clean up on failure
    if (fs.existsSync(tmpDir)) fs.rmSync(tmpDir, { recursive: true, force: true });

    console.error(`Installation failed: ${error.message}`);
    console.error("");
    console.error("Alternative installation methods:");
    console.error("  cargo install infiniloom");
    console.error("  brew tap Topos-Labs/infiniloom && brew install infiniloom");
    console.error("");
    console.error(`Manual download: https://github.com/${REPO}/releases/tag/v${VERSION}`);
    process.exit(1);
  }
}

install().catch(console.error);
