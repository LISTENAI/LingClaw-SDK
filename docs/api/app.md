# app · 应用信息

最低 API 版本：2。`app` 是运行器提供的全局表。

## app.api_version

最低 API 版本：2。

类型：`integer`。当前运行环境的 API 版本，可用于选择兼容的接口。

```lua
if app.api_version >= 3 then
    local now = clock.now()
end
```

## app.width

最低 API 版本：2。需要屏幕能力。

类型：`integer`。画布宽度，单位为像素；未启用屏幕时不存在。

## app.height

最低 API 版本：2。需要屏幕能力。

类型：`integer`。画布高度，单位为像素；未启用屏幕时不存在。

使用画布尺寸计算布局，例如 `screen.rect(0, 0, app.width, app.height, 0x101820)`。
