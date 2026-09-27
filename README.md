# LingClaw SDK

用 Lua 为 LingClaw 编写自己的小应用：时钟、专注计时器、小游戏、联网信息屏，
或任何适合你设备的轻量交互。

这里提供开发所需的 API 文档、示例和桌面模拟器。
你可以从零编写脚本，也可以用熟悉的编辑器修改已有应用，在电脑上调试后上传设备。

## 开始开发

### 1. 下载模拟器

前往 [Releases](https://github.com/LISTENAI/LingClaw-SDK/releases)，
下载对应系统和架构的压缩包：

| 系统 | 架构 | 文件名后缀 |
| --- | --- | --- |
| Windows | x64 | `windows-x64.zip` |
| Windows | ARM64 | `windows-arm64.zip` |
| Linux | x64 | `linux-x64.tar.gz` |
| Linux | ARM64 | `linux-arm64.tar.gz` |
| macOS | Apple Silicon | `macos-arm64.zip` |

解压后运行 `lingclaw-sdk.exe`、`lingclaw-sdk` 或 `LingClaw Simulator.app`。
包内附有 `examples/` 和完整 API 文档，无需安装 Rust。
Linux 需要图形桌面与 Vulkan 驱动；安装要求见[模拟器使用](docs/simulator.md)。

### 2. 打开示例并修改

在模拟器中打开 `examples/counter.lua`，点击画面后按空格键增加计数。
用 VS Code 等编辑器修改同一文件，保存后模拟器自动重载。

```lua
local count = 0

local function draw()
    screen.begin(0x152230)
    screen.text("Count: " .. count, 16, 16, 0xFFFFFF)
    screen.present()
end

function on_start() draw() end
function on_tick(dt_ms) end
function on_button_click(button_id)
    if button_id == "function" then
        count = count + 1
        draw()
    end
end
```

需要其他屏幕尺寸或按键时，在模拟器设置中复制设备配置并编辑。
你可以缩放预览、模拟按键、检查指示灯与蜂鸣音、测试联网请求和导出截图。

### 3. 上传设备

连接支持 ADB 调试的 LingClaw 设备，安装
[Android Platform Tools](https://developer.android.com/tools/releases/platform-tools) 后执行：

```sh
adb push counter.lua /miniapp/counter.lua
```

上传完成后应用立即运行。完整流程见[上手指南](docs/getting-started.md)。

## 文档

- [上手指南](docs/getting-started.md)：从第一份 Lua 到设备运行。
- [API 参考](docs/api/README.md)：按模块查阅参数、返回值和最低支持版本。
- [模拟器使用](docs/simulator.md)：快捷键、设备配置、重载和调试。
- [示例](examples/)：可直接打开的计数器和时钟。

## 参与开发

修改模拟器本身，请阅读 [simulator/README.md](simulator/README.md)。
问题反馈请使用 [Issues](https://github.com/LISTENAI/LingClaw-SDK/issues)，
附上最小复现脚本、系统版本和操作步骤。
贡献流程见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 许可证

本项目采用 [Apache-2.0](LICENSE)。第三方组件见 [THIRD_PARTY.md](THIRD_PARTY.md)。
