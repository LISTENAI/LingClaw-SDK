# 输入

最低 API 版本：2。按键由设备配置的 `hardware.input.buttons` 声明。

## on_button_click(button_id)

最低 API 版本：2。可选回调。

**作用：** 接收一次按键点击。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| button_id | string | 按键 ID，与设备配置中的 `id` 一致 |

**返回值：** 忽略。

每次短按释放产生一次点击。多按键共用此回调，通过 `button_id` 区分。
双击、连击可由应用记录相邻点击的间隔后判断，没有独立的双击回调。
达到设备配置中 `reserved_hold_ms` 的长按属于系统操作，不生成点击事件。

```lua
function on_button_click(button_id)
    if button_id == "function" then
        -- 执行功能键操作。
    elseif button_id == "left" then
        -- 执行自定义左键操作。
    end
end
```

示例中的按键 ID 需与目标设备配置一致。模拟器的键盘映射只负责产生对应按键事件。
