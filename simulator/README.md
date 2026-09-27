# LingClaw Simulator

基于 Rust 与 GPUI Kit 的桌面模拟器，用于运行、调试 LingClaw Lua 应用。
LVGL 负责画布绘制，GPUI 负责桌面交互。模拟器的 API 环境独立于设备硬件预设。

只需开发 Lua 应用时，从 [Releases](https://github.com/LISTENAI/LingClaw-SDK/releases)
下载模拟器即可。本文面向修改模拟器源码的开发者。

## 从源码运行

准备仓库 `rust-toolchain.toml` 指定的工具链，以及平台的 C/C++ 构建工具：

- macOS：Xcode Command Line Tools。
- Windows：Visual Studio C++ Build Tools、Windows SDK；ARM64 构建需安装 ARM64 工具。
- Ubuntu 24.04：安装以下开发依赖。

```sh
sudo apt-get install build-essential pkg-config clang cmake ninja-build \
  libfontconfig1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libvulkan-dev libssl-dev libxcb1-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libasound2-dev libudev-dev
```

在仓库根目录执行：

```sh
git submodule update --init --recursive
cargo run --locked
cargo run --locked -- examples/counter.lua
```

## 代码结构

| 路径 | 内容 |
| --- | --- |
| `src/main.rs` | 桌面窗口、文件重载和输入映射 |
| `src/runtime.rs` | Lua 环境、接口与生命周期 |
| `src/api.rs`、`src/api-defaults.json` | API 版本与默认额度 |
| `src/capabilities.rs`、`src/profiles.rs` | 能力验证与设备配置 |
| `src/network.rs`、`src/audio.rs` | HTTP 请求与音频播放 |
| `src/render.rs`、`native/` | LVGL Rust/C 桥接 |
| `profiles/` | 硬件预设 |
| `assets/` | 画布字形及许可 |
| `tests/` | 接口、像素、网络和重载回归 |
| `vendor/lvgl/` | 官方 LVGL Git submodule |

## 验证

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -- --headless examples/counter.lua target/counter.png
```

依赖真实音频设备或公网的测试默认忽略，按测试注释在具备条件的环境中单独运行。
界面交互需要在实际桌面环境验证；无窗口截图测试不覆盖窗口和键盘交互。

## 构建与发布

GitHub Actions 为 Windows x64/ARM64、Linux x64/ARM64 和 macOS Apple Silicon
分别运行格式检查、Clippy、测试、Release 构建和无窗口截图，并上传分发包。
推送 `v*` 标签时，在所有平台成功后创建草稿 Release，由维护者检查后发布。

每个分发包包含程序、Lua 示例、API 文档、许可证和 SHA-256 校验文件。
构建命令与依赖版本以 [CI 配置](../.github/workflows/check.yml) 为准。
本地 macOS 应用包可通过 `sh scripts/package-macos.sh` 在仓库根目录生成。
