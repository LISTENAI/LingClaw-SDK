# 第三方组件

| 组件 | 版本 / 来源 | 许可证 |
| --- | --- | --- |
| LVGL | [v8.4.0](https://github.com/lvgl/lvgl/tree/v8.4.0)，官方 Git submodule，固定发布提交 | [MIT](simulator/vendor/lvgl/LICENCE.txt) |
| 设备字形 | [ChillRoundGothic](https://github.com/Warren2060/ChillRoundGothic)，设备用位图子集 | [OFL-1.1](simulator/assets/OFL.txt) |
| GPUI Kit | Cargo.lock 中锁定版本 | Apache-2.0 |
| Rodio | Cargo.lock 中锁定版本 | MIT / Apache-2.0 |
| Reqwest / Tokio | Cargo.lock 中锁定版本 | MIT / Apache-2.0（Reqwest），MIT（Tokio） |
| Lua | mlua 启用 Lua 5.4 | MIT |

`simulator/assets/lingclaw-16.bin` 是用于复现设备显示效果的字形数据。
Rust 依赖的版本和来源由 Cargo.lock 固定，分发时须保留其许可信息。

完整 Rust 依赖许可见 [第三方声明](THIRD_PARTY_NOTICES.html)。
MPL-2.0 组件的原始源码可通过声明中的 crates.io 链接获取。
