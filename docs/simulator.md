# 模拟器使用

## 安装

从 [Releases](https://github.com/LISTENAI/LingClaw-SDK/releases) 下载对应系统与架构的压缩包并解压。
Windows 运行 `lingclaw-sdk.exe`，macOS 打开 `LingClaw Simulator.app`，Linux 运行 `./lingclaw-sdk`。

Linux 构建面向 Ubuntu 24.04 或更新的兼容系统，需要图形会话和 Vulkan 驱动。
Ubuntu 可安装运行依赖：

```sh
sudo apt-get install libfontconfig1 libxkbcommon0 libxkbcommon-x11-0 \
  libwayland-client0 libvulkan1 libxcb1 libxcb-shape0 libxcb-xfixes0 \
  libasound2t64 libudev1 libssl3t64
```

## 打开与重载

点击“打开 Lua”选择脚本，用 VS Code 等编辑器修改并保存。
模拟器默认自动重载，也可点击“重载”或按 Cmd/Ctrl+R 立即加载。
自动重载通常在保存后约 0.5 秒内完成。

每次重载会重新初始化 Lua 状态。源码启动失败时显示错误，
并保留仍可用的上一实例。右下角“事件”可查看调试信息。

## 画面与操作

- 使用 1×、2×、4× 切换预览比例。
- 点击相机按钮保存 PNG，输出尺寸与画布一致。
- 点击画面后使用键盘操作；按住快捷键时，对应按键高亮。
- 鼠标点击预览下方的按键也可触发点击事件。
- 暂停后点击“单步”推进一次 20 ms 的更新。
- 蜂鸣音通过电脑默认音频设备播放，底栏可切换静音。

## 设备配置

在设置顶部选择设备。使用 Mini 预设可直接开始调试。
需要其他屏幕尺寸、按键或指示灯时，点击“复制配置”，
在副本的“硬件”页修改；新增按键需填写代码标识与显示名称，
例如 `left` 和“左键”。

“按键映射”页列出当前设备的按键。点击右侧绑定按钮后按下键盘按键，
支持组合键；Esc 取消录入，“清除”解除绑定。
每个设备分别保存映射，切换设备时自动加载。

“高级”页可编辑完整的 capabilities，包括 API 版本和资源额度，
也可导入或导出 JSON。切换设备或修改硬件会重新启动应用。
配置与映射保存在系统用户配置目录，下次打开时继续使用。

硬件配置决定画布和输入输出设备，API 版本决定可调用的接口。
模拟器默认使用最新支持的公开 API；如需检查旧版本兼容性，
可在自定义配置中设置 `runtime.lua_sdk.version`。
各接口的最低版本见 [API 参考](api/README.md)。

## 网络与播报

HTTP 使用电脑的真实网络，支持 GET、POST、请求头及 HTTPS。
请求在后台执行，结果在应用更新时交付。暂停期间网络请求继续执行，
恢复后才调用 Lua 回调。

点击底栏“播报”打开侧栏，查看待播报文本并选择完成、中断或失败，
测试 `on_tts_result` 回调。模拟器不执行语音合成。

## 无界面截图

```sh
./lingclaw-sdk --headless examples/counter.lua screenshot.png
```

执行启动阶段和 50 次更新后保存画面。也可使用从设置中导出的配置：

```sh
./lingclaw-sdk --headless app.lua screenshot.png ./capabilities.json
```

Windows 使用 `lingclaw-sdk.exe`；macOS 可执行
`"LingClaw Simulator.app/Contents/MacOS/lingclaw-sdk"`。
命令不创建桌面窗口，Lua 异常时返回非零退出状态。
截图反映执行结束时的画面，不额外等待网络请求完成。

## 与设备联调

模拟器使用 LVGL 软件绘制、位图字形和 RGB565 颜色。
电脑的时区、网络、证书与音频设备可能与目标设备不同，
内存占用和任务调度也不等同于设备运行结果。

Mini 固件 3.0.2 及以上可将设备上正在运行的应用拉取到电脑，
在模拟器中打开源码、修改并验证后再传回设备。
完整步骤见[修改设备上的应用](getting-started.md#修改设备上正在运行的应用)。

上传后检查物理按键、音频和实际资源限制。
