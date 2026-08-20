// Signs the freshly built NSIS installer and generates latest.json.
//
// Usage: node scripts/sign-and-prepare-release.mjs
// Env:
//   TAURI_SIGNING_PRIVATE_KEY_PATH  private key path (default: ~/.tauri/dsh-desktop.key)
//   DSH_RELEASE_TAG                 GitHub release tag (default: v<package version>)
//   TAURI_SIGNING_PRIVATE_KEY_PASSWORD must be set by the caller when the key has a password.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const version = packageJson.version;
const releaseTag = process.env.DSH_RELEASE_TAG || `v${version}`;
const privateKeyPath = resolve(
  process.env.TAURI_SIGNING_PRIVATE_KEY_PATH || join(homedir(), ".tauri", "dsh-desktop.key"),
);
const nsisDir = join(root, "src-tauri", "target", "release", "bundle", "nsis");
const exeNames = existsSync(nsisDir) ? readdirSync(nsisDir).filter((name) => name.endsWith(".exe")) : [];

if (exeNames.length !== 1) {
  console.error(`Expected exactly one NSIS installer in ${nsisDir}, found ${exeNames.length}.`);
  process.exit(1);
}
if (!existsSync(privateKeyPath)) {
  console.error(`Signing key not found: ${privateKeyPath}`);
  console.error("Set TAURI_SIGNING_PRIVATE_KEY_PATH or create ~/.tauri/dsh-desktop.key first.");
  process.exit(1);
}
if (!process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
  console.error("TAURI_SIGNING_PRIVATE_KEY_PASSWORD is not set; refusing to sign interactively.");
  process.exit(1);
}

const installer = join(nsisDir, exeNames[0]);
console.log(`Signing ${exeNames[0]} for release ${releaseTag}…`);
execFileSync(
  "npm",
  [
    "run",
    "tauri",
    "--",
    "signer",
    "sign",
    "--private-key-path",
    privateKeyPath,
    installer,
  ],
  { cwd: root, stdio: "inherit", shell: true },
);

execFileSync(
  process.execPath,
  [join(root, "scripts", "build-latest-json.mjs"), version, releaseTag],
  { cwd: root, stdio: "inherit" },
);

console.log("Release artifacts ready:");
console.log(`  ${installer}`);
console.log(`  ${installer}.sig`);
console.log(`  ${join(root, "latest.json")}`);
