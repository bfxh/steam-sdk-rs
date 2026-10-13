# steam-sdk-rs

Steam SDK 的 Rust 集成工作仓。本仓聚合上游 [Noxime/steamworks-rs](https://github.com/Noxime/steamworks-rs) 的修复分支与本地分析文档。

## 结构

- `steamworks-rs/` — 代码子模块（submodule），追踪上游 + 本地修复分支
- `docs/` — 缺陷深挖分析、CI 本地证据、上游 PR 草稿

## 修复分支（在子模块内）

| 分支 | 主题 |
|------|------|
| `fix/call-result-handler-self-deadlock` | CallResult handler 自身死锁 |
| `fix/call-result-size-cap` | CallResult 结果大小上限 |
| `fix/callback-dispatch-reentrancy` | 回调分发重入 |
| `fix/callback-handle-drop-deadlock` | 回调 handle drop 死锁 |
| `fix/callback-multi-registration` | 回调重复注册覆盖 |
| `fix/cstring-nul-panic` | CString 空字节 panic |
| `fix/null-guard-unavailable` | 不可用符号的 null guard |
| `fix/test-tiering` | 测试分层 |
| `fix/ugc-builder-result` | UGC builder 结果处理 |
| `test/offline-accessor-coverage` | 离线访问器覆盖率 |
| `integration/stack8` | 八项修复集成堆栈 |
| `wip/callback-stream` | CallbackStream 原型 |

## 使用

```bash
git clone --recursive https://github.com/bfxh/steam-sdk-rs.git
cd steamworks-rs
cargo build
```
