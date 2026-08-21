// Signs the freshly built NSIS installer and generates bundle/latest.json.
//
// Usage: node scripts/sign-and-prepare-release.mjs
// Env:
//   TAURI_SIGNING_PRIVATE_KEY_PATH  private key path (default: ~/.tauri/dsh-desktop.key)
//   DSH_RELEASE_TAG                 GitHub release tag (default: v<package version>)
//   TAURI_SIGNING_PRIVATE_KEY_PASSWORD password (optional; falls back to
//     ~/.tauri/dsh-desktop.key.pass)
//
// The key password is taken from the environment variable when set, otherwise
// read from ~/.tauri/dsh-desktop.key.pass so the signing step never blocks on
// interactive input or requires re-entering the password in every shell.

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
const expectedName = `DeepSeek Harness Desktop_${version}_x64-setup.exe`;
const installerPath = join(nsisDir, expectedName);

// Old release installers may remain in the NSIS directory. Select the exact
// current-version artifact instead of requiring the directory to be empty.
if (!existsSync(installerPath)) {
  const available = existsSync(nsisDir)
    ? readdirSync(nsisDir).filter((name) => name.endsWith(".exe"))
    : [];
  console.error(`Current NSIS installer not found: ${installerPath}`);
  console.error(`Available installers: ${available.join(", ") || "(none)"}`);
  process.exit(1);
}
if (!existsSync(privateKeyPath)) {
  console.error(`Signing key not found: ${privateKeyPath}`);
  console.error("Set TAURI_SIGNING_PRIVATE_KEY_PATH or create ~/.tauri/dsh-desktop.key first.");
  process.exit(1);
}

let signingPassword = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
if (!signingPassword) {
  const passwordFilePath = join(homedir(), ".tauri", "dsh-desktop.key.pass");
  if (existsSync(passwordFilePath)) {
    signingPassword = readFileSync(passwordFilePath, "utf8").trim();
  }
}
if (!signingPassword) {
  console.error(
    "Signing password not found. Set TAURI_SIGNING_PRIVATE_KEY_PASSWORD or create ~/.tauri/dsh-desktop.key.pass.",
  );
  process.exit(1);
}

const installer = installerPath;
console.log(`Signing ${expectedName} for release ${releaseTag}…`);
const tauriCli = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
if (!existsSync(tauriCli)) {
  console.error(`Tauri CLI not found: ${tauriCli}`);
  console.error("Run npm install before signing release artifacts.");
  process.exit(1);
}
execFileSync(
  process.execPath,
  [tauriCli, "signer", "sign", "--private-key-path", privateKeyPath, installer],
  {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY_PASSWORD: signingPassword },
  },
);

execFileSync(
  process.execPath,
  [join(root, "scripts", "build-latest-json.mjs"), version, releaseTag],
  { cwd: root, stdio: "inherit" },
);

console.log("Release artifacts ready:");
console.log(`  ${installer}`);
console.log(`  ${installer}.sig`);
console.log(`  ${join(root, "src-tauri", "target", "release", "bundle", "latest.json")}`);
