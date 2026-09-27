# screen · 屏幕绘制

最低 API 版本：2。需要屏幕能力。颜色为整数 `0xRRGGBB`，低 24 位有效。
画面使用 RGB565，颜色会量化；像素原点在左上，x 向右、y 向下。

## screen.begin(rgb)

最低 API 版本：2。

**作用：** 清空待提交的绘制列表并设置背景。

- `rgb`：integer，背景颜色。
- 返回：无。

不会立即改变当前显示，需要调用 `screen.present()` 提交。

## screen.rect(x, y, w, h, rgb)

最低 API 版本：2。

**作用：** 绘制填充矩形。

| 参数 | 类型 | 含义 |
| --- | --- | --- |
| x、y | integer | 左上角，必须在画布内 |
| w、h | integer | 正宽度、正高度，像素 |
| rgb | integer | 填充颜色 |

返回无。矩形必须完全位于画布内，每帧最多 128 个。
后添加的矩形覆盖前面的矩形。

## screen.text(text, x, y, rgb)

最低 API 版本：2。

**作用：** 绘制 UTF-8 文本。

| 参数 | 类型 | 含义 |
| --- | --- | --- |
| text | string | UTF-8 文本，最多 63 字节 |
| x | integer | 左上角横坐标，0 到 `app.width - 1` |
| y | integer | 左上角纵坐标，0 到 `app.height - 16` |
| rgb | integer | 文字颜色 |

返回无。每帧最多 8 段，使用设备固定字形，名义字号 16 像素。
支持换行；实际行距和字宽由字形及 LVGL 决定。
超出屏幕的显示部分会裁剪，长文本也可能自动换行。

**文字始终位于矩形层上方**，即使 `screen.rect()` 在 `screen.text()` 后调用。

## screen.present()

最低 API 版本：2。

**作用：** 提交待绘制内容，使其成为当前画面。

无参数、无返回值。提交当前列表，后续无需重绘即可保持画面。
设备可以合并来不及显示的中间帧，不能用提交次数代替真实时间。

```lua
screen.begin(0x101820)
screen.rect(12, 12, app.width - 24, 36, 0x204030)
screen.text("Hello", 20, 20, 0xFFFFFF)
screen.present()
```
