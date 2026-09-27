# Lua API 参考

LingClaw 使用 Lua 5.4。模拟器默认提供最新支持的公开 API，当前为 API 4。
各接口的“最低 API 版本”表示该接口首次可用的版本。

## 模块

| 模块 | 用途 | 最低 API 版本 |
| --- | --- | --- |
| [app](app.md) | 查询 API 版本和画布尺寸 | 2 |
| [生命周期](lifecycle.md) | 初始化、定时更新、退出 | 2 |
| [输入](input.md) | 接收按键事件 | 2 |
| [screen](screen.md) | 绘制画面 | 2 |
| [led](led.md) | 控制指示灯 | 2 |
| [buzzer](buzzer.md) | 播放蜂鸣音 | 2 |
| [clock](clock.md) | 读取日历时间 | 3 |
| [storage](storage.md) | 保存应用数据 | 3 |
| [http](http.md) | 发起异步 HTTP 请求 | 4 |
| [json](json.md) | 编码和解析 JSON | 4 |
| [tts](tts.md) | 请求文本播报 | 4 |

## 使用约定

- [运行环境](runtime.md)说明可用 Lua 库、执行顺序和资源限制。
- `app.api_version` 是当前运行环境的 API 版本，与设备固件版本无关。
- 接口还可能需要屏幕、扬声器或网络能力；可用条件见各模块说明。
- 字符串长度限制以 UTF-8 字节计算，一个常见汉字通常占 3 字节。
- 参数非法可能产生 Lua 异常并终止应用。调用前应检查数据，
  不能依赖 `pcall` 或 `xpcall` 捕获错误，这两个函数不在运行环境中。
- 本文以方括号表示可选参数；“无返回值”表示不返回业务数据。
