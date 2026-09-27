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

安装 Android Platform Tools，连接支持 ADB 本地调试的设备：

```sh
adb devices
adb push counter.lua /miniapp/counter.lua
```

上传成功后脚本立即运行，替换当前应用。重新上传会重新初始化脚本。
目标文件名用于区分应用及其存档；源码不因上传而持久化，重启后需重新上传。

从支持源码拉取的设备取得当前运行脚本：

```sh
adb pull /miniapp/miniapp.lua ./device-app.lua
```

该路径返回当前应用的源码快照。没有应用运行时拉取失败。

## 接下来

按需要阅读各模块：

- [screen](api/screen.md)：绘制矩形与文字。
- [输入](api/input.md)：处理多个按键与连续点击。
- [clock](api/clock.md)：读取真实日期和时间。
- [storage](api/storage.md)：保存设置和进度。
- [http](api/http.md) 与 [json](api/json.md)：获取和处理网络数据。
- [tts](api/tts.md)：请求文本播报。

[API 索引](api/README.md)列出了完整模块及各自的最低支持版本。
