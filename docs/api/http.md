# http · 网络请求

最低 API 版本：4。需要 `hardware.network` 为 `true`，
且配置中存在 `runtime.lua_sdk.http`。

## http.request(options)

最低 API 版本：4。

**作用：** 创建异步 HTTP 请求。

`options` 是表，字段如下：

| 字段 | 类型 | 默认 / 约束 |
| --- | --- | --- |
| url | string | 必填；http:// 或 https://；最多 511 字节 |
| method | string | GET；支持 GET、POST |
| headers | table | 可选，字符串键值；最多 16 项、累计 1,024 字节 |
| body | string | 仅 POST 可用，最多 8,192 字节 |
| timeout_ms | integer | 默认 10,000，最多 60,000 毫秒 |
| max_response_bytes | integer | 默认 8,192，最多 32,768 字节 |

额度以能力描述为准。请求头按每行 `名称: 值\r\n` 计数；键不含冒号、换行和 NUL，值不含换行和 NUL。
返回整数请求 ID，表示已接受请求，不表示网络成功。

参数非法或并发请求超限会产生 Lua 异常。当前最多 4 个未完成请求；
应在应用中跟踪请求 ID，避免每个 tick 都新建请求。
不自动重试或跟随重定向。HTTPS 在模拟器中校验证书；目标设备的 TLS 设置可能不同。

## http.get(url[, options])

最低 API 版本：4。

**作用：** 以 URL 和可选参数创建请求。

- `url`：string，请求地址。
- `options`：可选表，常用字段为 headers、timeout_ms、max_response_bytes。
- 返回：请求 ID。

省略 method 时使用 GET，其他约束与 request 相同。

## http.cancel(request_id)

最低 API 版本：4。

**作用：** 取消本实例尚未交付结果的请求。

- `request_id`：integer，本实例返回的请求 ID。
- 返回：成功取消为 `true`，不存在或已经取消为 `false`。

成功取消后不再交付结果。实例退出和替换也会取消其请求。

## on_http_response(request_id, response)

最低 API 版本：4。

**作用：** 接收一次请求的最终结果。

可选回调，返回值忽略。

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| request_id | integer | 发起请求时返回的 ID |
| response | table | 完整响应或传输错误，结构如下 |

响应字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| status | integer | HTTP 状态码，完整收到响应时存在 |
| content_type | string | 响应的内容类型，未提供时为空字符串 |
| body | string | 响应体原始字节，可包含 NUL 和非 UTF-8 数据 |
| error | table | 传输失败时提供；`code` 和 `message` 均为字符串 |

成功收到完整响应：

```lua
{status = 200, content_type = "application/json", body = "{\"value\":42}"}
```

HTTP 4xx/5xx 也属于完整响应。传输失败：

```lua
{error = {code = "network_error", message = "请求失败"}}
```

先检查 `response.error`，再检查状态码，最后验证业务字段。
未定义回调时结果被释放，不会因此终止应用。
模拟器使用电脑网络执行真实请求。
真实联网保留响应体原始字节，包括 NUL 和非 UTF-8 数据；
错误码包括 `network_error`、`timeout`、`response_too_large` 和 `unavailable`。
模拟器 HTTPS 使用系统信任的证书根校验服务端证书。
