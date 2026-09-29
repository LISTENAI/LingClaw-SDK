# 上手指南

## 运行第一个应用

从 [Releases](https://github.com/LISTENAI/LingClaw-SDK/releases) 下载对应系统和架构的模拟器，
解压并运行。无需安装 Rust 或编译 SDK。

点击“打开 Lua”，选择压缩包里的 `examples/counter.lua`。
点击预览画面后，按空格键增加计数。
用 VS Code 等编辑器修改同一文件，保存后模拟器会自动重载。

## 编写 Lua

将下面的代码保存为 `counter.lua`，在模拟器中打开：

```lua
local count = 0

local function draw()
    screen.begin(0x152230)
    screen.text("Count: " .. count, 16, 16, 0xFFFFFF)
    screen.present()
end

function on_start()
    draw()
end

function on_tick(dt_ms)
end

function on_button_click(button_id)
    if button_id == "function" then
        count = count + 1
        draw()
    end
end
```

`on_start` 用于初始化，`on_tick` 用于定期更新，`on_button_click` 接收按键。
`on_tick` 必须定义，即使应用不需要动画。

`screen.begin` 开始一帧，`screen.present` 提交画面。
启用屏幕的应用启动时需提交一帧，静态画面无需重复绘制。
本例使用 Mini 预设的 `function` 按键；自定义设备应使用对应的按键 ID。

## 调整与验证

- 保存脚本后查看自动重载结果，或点击“重载”立即重新运行。
- 点击相机按钮保存截图；缩放比例不会改变截图尺寸。
- 暂停后使用“单步”检查动画的单次更新。
- 打开“事件”查看运行信息；错误提示会显示在窗口中。

在设置中选择设备。需要新增按键或更改屏幕时，点击“复制配置”，
通过“硬件”页编辑；在“按键映射”页点击绑定按钮并按下实际键盘按键。
使用 `app.width`、`app.height` 计算布局，可减少适配不同屏幕的工作。
详见[模拟器使用](simulator.md)。

## 上传与拉取

### 连接设备

安装 [Android Platform Tools](https://developer.android.com/tools/releases/platform-tools)，
通过 USB 连接支持 ADB 本地调试的设备，确认设备出现在列表中：

```sh
adb devices
```

### 上传本地脚本

```sh
adb push counter.lua /miniapp/counter.lua
```

上传成功后脚本立即运行，替换当前应用。重新上传会重新初始化脚本。
目标路径格式为 `/miniapp/<id>.lua`，其中 `<id>` 使用 1–121 个英文字母、数字、
连字符或下划线。相同目标路径对应同一个本地调试应用及其设备端存档。
源码仅保存在设备内存中，重启后需重新上传；通过 `storage` 保存的数据
遵循[存档接口](api/storage.md)的持久化和过期规则。

### 修改设备上正在运行的应用

Mini 固件 3.0.2 及以上支持通过 ADB 拉取当前应用的 Lua 源码。

1. 在设备上打开要修改的应用，并保持应用运行。
2. 将源码拉取到电脑：

   ```sh
   adb pull /miniapp/miniapp.lua ./device-app.lua
   ```

3. 在模拟器中打开 `device-app.lua`，用 VS Code 等编辑器修改同一文件。
   保存后模拟器自动重载，可检查画面、按键和运行结果。
4. 验证后将修改后的脚本传回设备：

   ```sh
   adb push ./device-app.lua /miniapp/device-app.lua
   ```

上传成功后设备立即运行修改后的脚本。后续修改可继续上传到同一路径。

拉取路径固定为 `/miniapp/miniapp.lua`，与原应用名称及上传时的文件名无关。
它返回拉取开始时的源码快照；没有应用运行时会失败。
请使用尚未存在的本地文件名保存，避免覆盖已有修改。

拉取内容只有 Lua 源码，不包含运行进度或存档。
回传时按目标文件名识别本地调试应用：上例的应用 ID 为 `local:device-app`。
只有原应用也使用同一 ID 时，才会使用同一份设备端存档；
模拟器中的存档也不会随源码上传。

## 接下来

按需要阅读各模块：

- [screen](api/screen.md)：绘制矩形与文字。
- [输入](api/input.md)：处理多个按键与连续点击。
- [clock](api/clock.md)：读取真实日期和时间。
- [storage](api/storage.md)：保存设置和进度。
- [http](api/http.md) 与 [json](api/json.md)：获取和处理网络数据。
- [tts](api/tts.md)：请求文本播报。

[API 索引](api/README.md)列出了完整模块及各自的最低支持版本。
