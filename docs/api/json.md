# json · JSON 编解码

最低 API 版本：4。此模块不要求网络能力。

## json.encode(value)

最低 API 版本：4。

**作用：** 将 Lua 值编码为 JSON 字符串。

**参数 `value`：** 布尔值、有限数字、字符串、表或 `json.null`。
返回字符串；失败返回 `nil, reason`，其中 `reason` 为字符串。

表使用全字符串键作为对象，或从 1 开始的连续整数键作为数组。
不接受循环引用、混合键、稀疏数组、函数、NUL 字符或非有限数字。

## json.decode(text)

最低 API 版本：4。

**作用：** 将 JSON 字符串解析为 Lua 值。

**参数 `text`：** string，待解析的 JSON 文本。返回对应 Lua 值；失败返回 `nil, reason`，其中 `reason` 为字符串。
JSON false 会返回 Lua false，不能用 `if not value` 判断解析是否失败。

## json.null

最低 API 版本：4。

**作用：** 在 Lua 数据中表示 JSON null。

类型：`lightuserdata`，仅作为 null 哨兵值使用。在表中保留 null 字段或数组元素；不要用 Lua nil 替代。
