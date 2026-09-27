# tts · 文本播报

最低 API 版本：4。要求联网、扬声器和 `runtime.lua_sdk.tts` 能力。

## tts.speak(text)

最低 API 版本：4。

**作用：** 提交一段文本，异步请求合成与播放。

- `text`：非空 UTF-8 纯文本，不能含 NUL；默认最多 512 字节。
- 返回：整数播报 ID，不表示已播放成功。

使用设备当前发音人与系统音量。参数非法会产生 Lua 异常。
不允许在源码执行、`on_start()`、启动的第一次 tick 或退出期间调用。
应由之后的按键或正常 tick 触发。

同一时刻只有一个播报任务。对话或其他播报占用时通过结果回调返回 `failed/busy`，
不抢占、不自动排队。应等待当前结果后再发起下一次播报。

## tts.cancel(speech_id)

最低 API 版本：4。

**作用：** 取消本实例发起的播报。

- `speech_id`：integer，本实例申请的 ID。
- 返回：成功取消 `true`，不存在或已取消 `false`。

只影响本实例的播报，不能停止系统对话或闹钟。成功取消后不再回调。

## on_tts_result(speech_id, result)

最低 API 版本：4。

**作用：** 接收播报完成、失败或中断的结果。

可选回调，返回值忽略。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| speech_id | integer | `tts.speak` 返回的播报 ID |
| result | table | 本次播报的最终结果 |

`result` 字段：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| status | string | completed、failed、interrupted |
| error | table / nil | 失败时可包含 code、message 两个字符串 |

`completed` 表示播放完毕；`interrupted` 表示被唤醒、闹钟等中断；
`failed` 表示请求或播放失败。未定义回调也可请求播报，结果会自动释放。

```lua
local speech
function on_button_click(id)
    if id == "function" and not speech then
        speech = tts.speak("计时结束")
    end
end
function on_tts_result(id, result)
    if id ~= speech then return end
    speech = nil
    -- 根据 result.status 更新自己的状态。
end
```

示例需配合 `on_tick` 及必要的启动画面使用。
模拟器显示文本和完成/中断/失败按钮，不连接真实语音服务。
