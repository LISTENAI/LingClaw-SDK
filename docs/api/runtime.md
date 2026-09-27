# 运行环境

LingClaw 在受限的 Lua 5.4 环境中执行应用。
应用信息见 [app](app.md)，启动和回调顺序见[生命周期](lifecycle.md)。

## 可用 Lua 库

开放基础函数以及 `table`、`string`、`math`、`utf8`。
不提供 `io`、`os`、`package`、`debug`、`coroutine`、动态加载、文件和原始套接字。
`collectgarbage`、`dofile`、`load`、`loadfile`、`print`、`warn`、`pcall`、`xpcall`、
`setmetatable` 不可用。

## 资源限制

默认源码上限为 65,536 字节，单实例 Lua 堆为 393,216 字节。
源码必须是文本，不接受 Lua 字节码。
源码执行预算为 500,000 条指令，回调预算为 250,000 条指令。
运行器还检查执行耗时：源码约 150 ms、回调约 500 ms。
源码与堆额度由 `runtime.lua_sdk` 配置提供。

限制是防止脚本阻塞系统的上限，不是每帧可用的常规工作量。
时间预算无法抢占任意 C 库函数；模拟器也不是运行不可信代码的安全隔离环境。
应尽量短时间完成回调，将动画进度存入变量，在后续 tick 中推进。
