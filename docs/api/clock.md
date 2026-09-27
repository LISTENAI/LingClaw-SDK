# clock · 日历时间

最低 API 版本：3。

## clock.now()

最低 API 版本：3。

**作用：** 读取当前 UTC 时间。

无参数。返回 UTC Unix 时间戳（integer，秒），未校时返回 `nil`。

## clock.localtime()

最低 API 版本：3。

**作用：** 读取当前本地日期与时间。

无参数。未校时返回 `nil`，否则返回包含以下字段的 `table`：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| year | integer | 完整年份 |
| month | integer | 1–12 |
| day | integer | 1–31 |
| hour | integer | 0–23 |
| min | integer | 分钟，0–59 |
| sec | integer | 秒，0–59 |
| wday | integer | 0 为周日，6 为周六 |
| utc_offset | integer | 相对 UTC 的秒数 |

模拟器使用电脑的系统时间和时区。日历时间不随模拟器单步推进。
