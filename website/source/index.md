---
title: DeepSeek Harness Desktop
---

<div class="hero">
  <div class="hero-content">
    <h1 class="hero-title">DeepSeek Harness Desktop</h1>
    <p class="hero-subtitle">把 <strong>DeepSeek Harness</strong> 装进一个真正的桌面应用</p>
    <p class="hero-desc">不用再守着浏览器标签页——一个图标，双击打开，关掉窗口它还在后台安静运行。</p>
    <div class="hero-actions">
      <a href="https://github.com/luanshang/DeepSeek-Harness-Desktop-Built-with-Tauri/releases/latest" class="btn btn-primary">⬇️ 立即下载</a>
      <a href="https://github.com/luanshang/DeepSeek-Harness-Desktop-Built-with-Tauri" class="btn btn-secondary">查看源码</a>
    </div>
    <img src="/DeepSeek-Harness-Desktop-Built-with-Tauri/images/app-running.png" alt="应用运行截图" class="hero-screenshot">
  </div>
</div>

## 功能亮点 {#features}

<div class="features-grid">

<div class="feature-card">
  <div class="feature-icon">🚀</div>
  <h3>双击即用</h3>
  <p>打开应用，自动帮你把后台服务准备好，不需要敲命令行、不需要搞懂端口是什么。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">🚪</div>
  <h3>关窗不等于退出</h3>
  <p>点右上角的关闭按钮只是把窗口藏起来，工作还在继续；托盘图标右键才是真的退出。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">🔄</div>
  <h3>数据与网页版通用</h3>
  <p>所有会话、配置都存在同一个地方，网页版和桌面版随便切换，互不冲突。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">📁</div>
  <h3>文件面板</h3>
  <p>右侧一键唤出文件目录树，点开任意文件直接预览、编辑、保存，改动会用颜色标出来。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">🧩</div>
  <h3>插件市场</h3>
  <p>浏览、搜索、安装社区插件，安装前会先给你看清楚要执行的确切命令，确认后才会真正安装。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">💻</div>
  <h3>内置终端</h3>
  <p>需要跑个命令的时候，不用再额外开一个终端窗口，应用里直接就有。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">🛡️</div>
  <h3>崩溃自恢复</h3>
  <p>后台服务万一意外挂掉，应用会自动帮你重启一次；实在起不来也会告诉你原因。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">🔄</div>
  <h3>自动更新</h3>
  <p>打开应用时自动检查新版本，一键装上，不用跑去发布页面翻。</p>
</div>

<div class="feature-card">
  <div class="feature-icon">⚡</div>
  <h3>体积小、开得快</h3>
  <p>走的是系统自带的浏览器内核，不捆绑 Chromium，装包小很多，打开也更快。</p>
</div>

</div>

## 应用截图 {#screenshots}

<div class="screenshots-gallery">

<figure>
  <img src="/DeepSeek-Harness-Desktop-Built-with-Tauri/images/app-installer.png" alt="安装器截图">
  <figcaption>安装程序——简洁的向导式界面</figcaption>
</figure>

<figure>
  <img src="/DeepSeek-Harness-Desktop-Built-with-Tauri/images/app-boot.png" alt="启动页截图">
  <figcaption>启动页——加载中状态</figcaption>
</figure>

<figure>
  <img src="/DeepSeek-Harness-Desktop-Built-with-Tauri/images/app-running.png" alt="运行截图">
  <figcaption>主界面——文件面板与终端</figcaption>
</figure>

</div>

## 下载 {#download}

<div class="download-section">

| 平台 | 安装包 | 说明 |
|---|---|---|
| Windows (x64) | `.exe` | 支持自动更新，双击安装即可 |

> 目前仅提供 Windows 版本；macOS / Linux 支持尚未启用。
> 首次运行若弹出 Windows SmartScreen 提示，点「更多信息 → 仍要运行」即可。

<a href="https://github.com/luanshang/DeepSeek-Harness-Desktop-Built-with-Tauri/releases/latest" class="btn btn-primary btn-lg">⬇️ 前往 Releases 页面下载</a>

</div>

## 常见问题 {#faq}

<div class="faq-list">

**这是官方产品吗？**
不是。DeepSeek Harness 本体由官方维护，这个桌面壳是社区做的第三方封装。

**我的数据安全吗？**
应用里的网页界面运行在隔离沙箱里，没有权限访问你电脑上的任何东西。唯一会读写本地文件的是桌面壳自带的文件面板——这是一个独立功能，跟网页那部分完全隔离。

**能不能带着走，不用联网也能用？**
应用本身可以离线打开，但里面加载的 DeepSeek Harness 服务是否需要联网取决于你配置的模型服务。

**想参与开发或者自己编译？**
欢迎，查看 [开发指南](https://github.com/luanshang/DeepSeek-Harness-Desktop-Built-with-Tauri/blob/main/docs/DEVELOPMENT.md)。

</div>