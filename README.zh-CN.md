# OctosCode

[English](README.md) | 简体中文

OctosCode 是使用 Makepad 和 Octoscript 构建的 [Octos](https://github.com/octos-org/octos)
原生桌面客户端。它与 [octoscode-web](https://github.com/octos-org/octoscode-web) 一起，
为编程对话、工作区、模型提供商和会话权限提供操作界面。

独立版无需 OctoSense 即可运行。默认构建不依赖 OctoSense 的外壳、内核、配置库或模块宿主。
应用通过 WebSocket 连接单独运行的 `octos serve`，使用 `octos-ui/v1alpha1` 协议（JSON-RPC 2.0）。

## 试用候选版本

前往 **[v0.1.0-rc.1 发布页](https://github.com/octos-org/octoscode-app/releases/tag/v0.1.0-rc.1)**
查看下载和平台支持情况。这是供测试使用的预发布版本。

| 平台 | 安装包 |
|---|---|
| macOS，Apple Silicon | `OctosCode-macos-arm64.zip` |
| Linux，x86_64 | `OctosCode-linux-x86_64.tar.gz`（Ubuntu 22.04 或更新版本） |
| Windows，x86_64 | 已成功构建；原生启动及数据目录验证完成后再发布 |

在 macOS 上，解压后将 `OctosCode.app` 移入“应用程序”并打开。应用包自带界面资源，
运行时无需安装 Rust、保留源码目录或安装 OctoSense。该版本采用临时签名（ad-hoc），
尚未经过 Apple 公证。如果 macOS 阻止打开从本仓库下载的应用，请先尝试打开一次，
再进入 **系统设置 → 隐私与安全性 → 仍要打开**。

请按照 **[快速入门](docs/QUICKSTART.zh-CN.md)** 启动 Octos 服务器、连接客户端并配置模型提供商。
桌面客户端下载包不包含服务器。

## 在 macOS 上构建

先安装 Xcode 命令行工具、稳定版 Rust 和 Python 3，然后运行：

```sh
git clone https://github.com/octos-org/octoscode-app.git
cd octoscode-app
tools/build-macos.sh --release --package
open target/macos-app/OctosCode.app
```

脚本会准备固定版本的渲染依赖并构建应用。`--release` 生成优化后的可执行文件；
`--package` 始终使用优化的 `app-bundle` 配置生成应用包。
日常使用或性能测试应使用优化构建。不加这两个参数时，脚本生成调试版可执行文件。

如需嵌入 OctoSense，可运行 `tools/build-macos.sh --octosense --release`。
`--release` 同时作用于独立版和 OctoSense 宿主。
依赖、产物路径、宿主启动命令和构建问题排查详见 [macOS 构建指南（英文）](docs/BUILD-macos.md)。

## 使用应用

- 使用服务器地址和访问令牌、新的配对链接，或“使用本地单人服务器”进行连接。
- 在“设置 → 模型提供商”中配置提供商，然后选择工作区和会话。
- 拖动左侧栏右边缘调整宽度；折叠后显示图标栏，再次展开会恢复之前的宽度。
- 连接配置、凭据、草稿和偏好保存在 `~/.octoscode`。

从命令行启动时，可传入 `--server <url> --token-file <path> --workspace <directory>`。
示例见[快速入门](docs/QUICKSTART.zh-CN.md#使用已有令牌文件启动)。
