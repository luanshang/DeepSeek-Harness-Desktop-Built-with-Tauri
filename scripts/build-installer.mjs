import { cpSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const version = packageJson.version;
const payloadSource = join(root, "src-tauri", "target", "release", "bundle", "nsis", `DeepSeek Harness Desktop_${version}_x64-setup.exe`);
const payload = join(root, "installer", "payload.exe");
const hostRoot = join(root, "installer-host");
const hostManifest = join(hostRoot, "Cargo.toml");
const outputDir = join(root, "src-tauri", "target", "release", "bundle", "nsis");
const output = join(outputDir, `DeepSeek Harness Desktop_${version}_x64-setup.exe`);

function runNodeScript(script, args, cwd = root) {
  console.log(`\n> node ${script} ${args.join(" ")}`);
  execFileSync(process.execPath, [script, ...args], { cwd, stdio: "inherit" });
}

function run(command, args, cwd = root) {
  console.log(`\n> ${command} ${args.join(" ")}`);
  execFileSync(command, args, { cwd, stdio: "inherit" });
}

if (!existsSync(join(root, "src-tauri", "resources", "runtime", "node.exe"))) {
  console.warn("Warning: bundled node.exe is missing; run npm run fetch:node first.");
}

// First build the ordinary Tauri NSIS package as an invisible payload. Its
// existing hooks keep uninstall, shortcuts, and Explorer integration intact.
runNodeScript(join(root, "node_modules", "@tauri-apps", "cli", "tauri.js"), ["build", "--bundles", "nsis"]);
if (!existsSync(payloadSource)) throw new Error(`Payload was not generated: ${payloadSource}`);
cpSync(payloadSource, payload, { force: true });

// Compile the separate WebView2/Tauri CSS bootstrapper. Its Rust binary embeds
// installer/payload.exe at compile time, so the setup file is self-contained.
mkdirSync(join(hostRoot, "icons"), { recursive: true });
cpSync(join(root, "src-tauri", "icons", "icon.ico"), join(hostRoot, "icons", "icon.ico"), { force: true });
run("cargo", ["build", "--release", "--manifest-path", hostManifest]);
const hostExe = join(root, "installer-host", "target", "release", "dsh-installer.exe");
if (!existsSync(hostExe)) throw new Error(`Installer host was not generated: ${hostExe}`);
mkdirSync(outputDir, { recursive: true });
cpSync(hostExe, output, { force: true });
console.log(`\nBranded setup generated: ${output}`);
