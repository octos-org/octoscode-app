# OctosCode 快速入门

[English](QUICKSTART.md) | 简体中文 · [README](../README.zh-CN.md)

OctosCode 是独立桌面**客户端**，还需要连接一个 Octos 服务器。服务器可以运行在同一台机器
上，也可以由管理员提供远程地址。OctoSense 是可选组件。

## 1. 安装应用

前往 [v0.1.0-rc.1 发布页](https://github.com/octos-org/octoscode-app/releases/tag/v0.1.0-rc.1)
查看可用的平台下载。macOS ARM64 应用包已通过验证；Linux 正在准备打包，Windows 暂未确认。

在 Apple Silicon Mac 上：

1. 下载 `OctosCode-macos-arm64.zip`。
2. 解压后将 `OctosCode.app` 移入 `/Applications` 或 `~/Applications`。
3. 打开应用。这个预发布版本采用临时签名（ad-hoc），尚未经过 Apple 公证。
   如果 macOS 阻止打开从本仓库下载的应用，请先尝试打开一次，再进入
   **系统设置 → 隐私与安全性 → 仍要打开**。

Linux 应用包发布后，可下载面向 Ubuntu 22.04 或更新版本 x86_64 环境的
`OctosCode-linux-x86_64.tar.gz`，解压后在图形桌面中启动：

```sh
tar -xzf OctosCode-linux-x86_64.tar.gz
cd OctosCode
./octoscode
```

请保持 `makepad/` 目录与可执行文件位于同一目录。若 Windows 应用包发布，请解压
`OctosCode-windows-x86_64.zip`，打开 `OctosCode/octoscode.exe`，并同样保留旁边的
`makepad/` 目录。最终的平台验证状态以发布说明为准。

下载包包含客户端及界面资源，不会自动启动服务器。运行应用无需源码目录、Rust 或 OctoSense。

## 2. 启动或找到服务器

已有运行中的服务器时，可直接跳到第 3 步。若要在本机运行，请先安装稳定版 Rust 和系统所需的
原生编译工具（macOS 上为 Xcode 命令行工具），然后安装 Octos CLI。
下面的命令使用客户端协议依赖所固定的服务器版本：

```sh
cargo install --git https://github.com/octos-org/octos \
  --rev a6ea8505170735f191a12bb47629a6728415d417 --features api octos-cli
octos serve --solo --host 127.0.0.1 --port 50190
```

请保持这个终端运行。`api` 功能已包含 WebSocket 端点，无需额外启用 WebSocket。
`--solo` 开启下文使用的本地单人登录；此模式应仅绑定回环地址，不放在反向代理后面。

连接他人管理的服务器时，请获取服务器 URL，以及访问令牌或新的配对链接。
服务器配置详见 [Octos 仓库](https://github.com/octos-org/octos)。

## 3. 连接并发送第一条消息

1. 在“连接到 Octos”界面输入服务器地址。使用上述本机启动命令时，地址为
   `http://127.0.0.1:50190`，然后选择“使用本地单人服务器”。
2. 若服务器要求令牌，请输入对应的访问令牌并点击“连接”。
   也可选择“改用链接配对”，输入服务器提供的新配对链接。
3. 按提示完成模型提供商配置；也可打开“设置 → 模型提供商”，添加提供商、API 密钥及账号可用的模型。
4. 选择工作区，创建或打开会话，然后发送消息。连接远程服务器时，工作目录位于服务器所在的机器上。

拖动侧栏右边缘可调整宽度；折叠按钮将侧栏切换为图标栏，再次展开会恢复之前的宽度。
应用会将连接信息和偏好保存在 `~/.octoscode`。

### 使用已有令牌文件启动

如果已有保存服务器访问令牌的文件，可以使用以下方式启动，避免把令牌值直接写在命令行里。
使用这些启动参数前，请先退出已经打开的应用。以下是 macOS 应用包的示例：

```sh
open /Applications/OctosCode.app --args \
  --server http://127.0.0.1:50190 \
  --token-file /path/to/server.token \
  --workspace /path/to/project
```

请替换为实际地址和路径。`--token-file` 指向客户端机器上的文件，应用会为该服务器保存令牌。
`--workspace` 选择服务器上的工作目录。直接运行 `octoscode` 可执行文件时，也可使用这三个参数。

## 4. 在 macOS 上构建优化版本

需要 macOS、Xcode 命令行工具（`xcode-select --install`）、稳定版 Rust、Python 3、Git，
以及访问 GitHub 和 crates.io 的网络连接。

```sh
git clone https://github.com/octos-org/octoscode-app.git
cd octoscode-app
git checkout v0.1.0-rc.1
tools/build-macos.sh --release --package
open target/macos-app/OctosCode.app
```

在 Apple Silicon 上，产物包括 `target/release/octoscode`、`target/macos-app/OctosCode.app`
和 `target/macos-app/OctosCode-macos-arm64.zip`。
若设置了 `CARGO_TARGET_DIR`，产物路径将使用该目录代替 `target`。

应用包始终使用优化的 `app-bundle` 配置。不加 `--release` 时，单独的可执行文件为调试构建；
评估性能时请使用优化构建。若要构建默认分支而非这个 RC，请省略 `git checkout` 命令。

如需同时构建可选的 OctoSense 宿主：

```sh
tools/build-macos.sh --octosense --release
MAKEPAD_WM_TEST_APP=octoscode OCTOSCODE_DESIGN_DIR="$PWD/design" \
  .forks/octosense-host/target/release/octosense --module octoscode
```

这里的 `--release` 同时作用于两个可执行文件。独立版下载包不包含该可选宿主。
完整构建流程见 [macOS 构建指南（英文）](BUILD-macos.md)。

### Linux 和 Windows 本机打包

[桌面构建工作流](../.github/workflows/desktop-packages.yml) 列出了 Ubuntu 22.04 所需的
原生依赖及 Windows 构建环境。安装对应依赖、Rust、Python 3 和 Git 后，在克隆的仓库中执行
以下命令（Windows 请使用 Git Bash）：

```sh
bash tools/prepare-makepad-fork.sh
bash tools/prepare-octoscript-makepad-fork.sh
python3 tools/package-desktop.py
```

Windows 上若 Python 3 命令为 `python`，请相应替换。脚本使用优化的 `app-bundle` 配置，
为当前平台生成便携归档和 SHA-256 校验文件，输出到 `target/desktop-packages/`。
该脚本不进行交叉编译。

## 常见问题

| 现象 | 检查方法 |
|---|---|
| 无法连接 | 确认 `octos serve` 仍在运行，并检查地址和端口是否一致。 |
| 令牌被拒绝 | 使用该服务器当前的访问令牌；若服务器以 `--solo` 启动，请选择本地单人登录。 |
| 配对链接已使用 | 在服务器端生成新链接；已用过的链接不能重复使用。 |
| 已连接，但模型调用失败 | 在设置中检查提供商、模型和 API 密钥。服务器访问令牌与模型提供商的 API 密钥是两种不同的凭据。 |
| 侧栏或输入很慢 | 使用发布的应用包或 `--release` 可执行文件；调试构建会明显更慢。 |
| 复制可执行文件后缺少字体或图标 | 请复制打包后的 `.app` 或发布 ZIP；直接从源码构建的单个可执行文件不是可移植应用包。 |
| macOS 阻止打开 | 对从本仓库下载的应用，按第 1 步在系统设置中单独批准打开。 |
