# 发布指南（Release）

> 本文档说明如何把新版本发布到 GitHub Releases，让普通用户能下载安装，并启用应用内自动更新。
> 面向"准备发版的人"。开发 / 构建细节见 [DEVELOPMENT.md](DEVELOPMENT.md)。

## 每次发布要产出什么

| 文件 | 来源 | 用途 |
|---|---|---|
| `DeepSeek Harness Desktop_<版本>_x64-setup.exe` | `npm run bundle` | 用户下载安装 |
| `<同名>.exe.sig` | `tauri signer sign` | 自动更新的版本签名 |
| `latest.json` | `node scripts/build-latest-json.mjs` | 自动更新的版本清单 |

如果**不做自动更新**，只需上传安装包那一个文件，签名和清单两步都可以跳过。

## 一次性准备：签名密钥（必须做！）

自动更新用 minisign 密钥对做签名校验。**当前 `src-tauri/tauri.conf.json` 里的 `pubkey` 还是原作者 xiincs 的，必须换成你自己生成的**，否则你发的新版本永远通不过校验，用户点"更新"会失败。

```powershell
npm run tauri -- signer generate -w "$env:USERPROFILE\.tauri\dsh-desktop.key"
```

- 命令会打印一串 public key（base64，`dW50cnVzdGVk...` 开头），把它填进 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。
- 私钥文件 `~/.tauri/dsh-desktop.key` 和它的密码**务必备份**。私钥一旦丢失，就再也无法签名更新，只能换新密钥对——而老用户装过的版本只认旧公钥，等于全部要重装。
- 私钥在仓库外（`~/.tauri/`），不会被提交，放心；但别把它拷进项目目录。

## 发版步骤（每次发布照着做）

### 1. 检查内置 dsh 版本

```powershell
npm run check:dsh-version
```

上游 DeepSeek Harness 还在快速发 RC。脚本会对比写死在 `src-tauri/src/server.rs`（`DSH_VERSION_DEFAULT`）和 `scripts/prepare-runtime.mjs` 里的默认版本号与 npm 上的最新版。落后了就同步两处（**必须一致**），直到检查通过。

### 2. 升外壳版本号

`src-tauri/tauri.conf.json` 的 `version` 和 `package.json` 的 `version` 改成同一个新版本号（如 `1.5.1`）。

> 注意两条独立的版本轴线：这里升的是**外壳版本**（自动更新只认它），和内置 dsh 运行时的版本互不相干，别混。

### 3. 构建、签名并生成更新清单

先在当前 PowerShell 会话设置私钥密码：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "你的私钥密码"
```

然后执行一条命令：

```powershell
npm install
npm run bundle
```

`npm run bundle` 会依次执行：

1. 准备 Node 和内置 dsh 运行时；
2. 构建 NSIS 安装器；
3. 使用 `~/.tauri/dsh-desktop.key` 自动生成 `.exe.sig`；
4. 自动生成 `src-tauri/target/release/bundle/latest.json`。

如私钥不在默认位置，可设置：

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "D:\secure\dsh-desktop.key"
$env:DSH_RELEASE_TAG = "v1.5.1" # 默认自动使用 v<package.json version>
```

最终产物：

- `src-tauri/target/release/bundle/nsis/DeepSeek Harness Desktop_<版本>_x64-setup.exe`
- 同目录的 `.exe.sig`
- `src-tauri/target/release/bundle/latest.json`

脚本拒绝在未设置 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 时签名，避免构建过程中出现交互式密码输入。

### 6. 提交并打 tag

```powershell
git add -A
git commit -m "Release v1.5.1"
git tag v1.5.1
git push
git push origin v1.5.1
```

### 7. 创建 GitHub Release

仓库页 → **Releases** → **Draft a new release** → 选 tag `v1.5.1` → 写更新说明 → 上传三个文件：

- `DeepSeek Harness Desktop_1.5.1_x64-setup.exe`
- `DeepSeek Harness Desktop_1.5.1_x64-setup.exe.sig`
- `latest.json`

→ **Publish release**。

> ⚠️ 资产里必须有一个**恰好叫 `latest.json`** 的文件——自动更新的 endpoint
> 是 `.../releases/latest/download/latest.json`，会去"最新 release"里找名为 `latest.json` 的资产。

### 8. 验证

- 在自己机器上装一个**旧版本**，启动看它是否提示新版本并成功更新；
- 或直接装新包，确认安装、启动、托盘退出都正常。

### 9. 顺手更新仓库门面

仓库页右侧 **About ⚙️**：填描述、Website（可填 Releases 页地址）、Topics
（`tauri` `rust` `deepseek` `deepseek-harness` `desktop-app` 等）。

## 常见坑

- **签名后别动 exe 和 sig**：`build-latest-json.mjs` 只认 `nsis` 目录里唯一一个 `*.exe.sig`，步骤 4 → 5 之间保持产物原样。
- **latest.json 里的版本 / tag 要和 Release 一致**：脚本第 5 步的 `v<版本>` 就是下载 URL 里的 tag，写错 = 用户点更新 404。
- **`createUpdaterArtifacts` 保持 `false`**：项目特意关了它并用手动脚本——tauri-action 对带空格的 productName 生成 latest.json 有上游 bug（见 `build-latest-json.mjs` 头部注释），本地手动流程最稳。
- **签名时用 `--private-key-path` 传私钥文件**：`-k`/`--private-key` 只接受密钥字符串，传文件路径会报 `failed to decode base64 secret key`。密码用环境变量 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 传入，避免交互输入出错。
- **私钥别进仓库**：发现 `.key` 进了项目目录就立即撤销换新。
- **内置运行时随外壳一起更新**：bundle 打出来的包把 dsh 运行时揉在安装包里，外壳更新会自动连带更新运行时（前提是步骤 1 的版本检查做过），不需要另搭一套运行时更新。

## 可选：CI 自动化

有兴趣的话可以让 GitHub Actions 在打 tag 时自动构建并上传 Release。注意 tauri-action 有上面的 latest.json 上游 bug，workflow 里应复用 `build-latest-json.mjs` 生成清单，不能完全依赖 tauri-action。