# 缺陷 #4 深度分析：接口指针仅 `debug_assert!` 判空，release 下裸指针解引用

> 依据核查清单完成：A1 复现 ✓ / A2 计数可复算 ✓ / A3 上游核查 ✓ / B1 仓内先例 ✓ / B2 影响面分层 ✓ / C1 重复检索 ✓
> 行号对照工作区（见主文档头部基准声明）。

## 结论

**缺陷成立，严重度：高。**

`Client` 的 17 个接口 getter 中 16 个用 `debug_assert!(!ptr.is_null())` 判空（timeline 例外，用 `disabled` 运行时标记——即 B1 先例本身），该断言在 `--release` 下被编译器完全删除；若接口指针为 `null`，结构体直接持有空指针，后续方法调用即构成未定义行为——实测 release 下 `Illegal instruction`（退出码 132）。

**修复已有零破坏先例**：`Timeline` 用 `disabled: bool` 标记 + 方法入口 `if self.disabled { return; }`（`src/timeline.rs:64/77/94/116`），不改公开签名即 release 安全。

---

## A. 取证过程

### A1. 复现证据

`repro/null_guard_test.rs` 镜像 `src/lib.rs:351-360`（`utils()`）的模式：`debug_assert!` 守卫 + 裸指针解引用。

**debug 模式**：
```
thread 'main' panicked at null_guard_test.rs:15:5:
assertion failed: !ptr.is_null()
```
构造阶段即 panic，null 不可达。

**release 模式（-O）**：
```
*** Illegal instruction (core dumped) ***
```
`debug_assert!` 被编译掉，构造静默通过，解引用 null 触发 SIGILL/退出码 132。**同一行源码在两种模式下行为完全不同**——这正是 `debug_assert!` 不能用作 release 安全守卫的原因。

### A2. 计数可复算

| 口径 | 数值 | 命令 |
|------|------|------|
| `Client` 公开接口 getter | 17（16 用 debug_assert + timeline 用 disabled） | 逐 getter awk 对账（defect-07 深挖时发现原 15 漏了 networking_messages/sockets/utils 三个 `_SteamAPI_` 中缀访问器） |
| `debug_assert!(!x.is_null())` in lib.rs | 17 | `grep -rn "debug_assert!(!.*\.is_null())" src/lib.rs \| wc -l` |
| 全仓判空断言 | 23 | lib.rs 17 + server.rs 6（`grep -rn … src/ \| wc -l`） |
| GameServer 侧 getter | 6 | server.rs:537-596（ugc/utils/networking×2/sockets/apps），**已逐一确认全为 getter** |

**核心面是 16 个 debug_assert getter + 6 个 GameServer getter（timeline 不需修，已是正确先例）。**

### A3. 上游核查与可达性

`Client::init()` 文档明确列出失败情形（Steam 未运行/app ID 未知/无许可证），返回 `Err` 时拿不到 `Client`，这条路安全。**真正的 null 来源**：

1. **初始化后接口失效**：Steam 客户端崩溃/重登期间部分接口暂时返回 null，已获取的句柄不会刷新。
2. **非 Steam 启动**：`steam_appid.txt` 伪造或 demo/无许可证场景下，具体接口可能返回 null。
3. **模块未加载**：Valve C SDK 中接口获取函数在未初始化/未加载时返回 null 是**正常契约行为**，调用方本应判空——Rust 绑定把判空推给 `debug_assert!`，等于 release 下把责任完全丢给调用方。

---

## C1. 重复检索

```bash
gh issue list --repo Noxime/steamworks-rs --state all \
  --search "debug_assert OR is_null OR interface OR panic" --limit 15
```

- Issue #9 `Panicking in a callback…without debug assertions`（CLOSED 2019）——**相关但不同**：针对回调 unwind 检查被 `#[cfg(debug_assertions)]` 门控，非接口判空。
- Issue #172（achievement 图标 assert）、#253（死锁）、#291/#292/#327（回调/客户端销毁崩溃）——均无关。

**结论：无重复报告**，具备单独提 Issue 条件（提之前仍需 A5：确认上游无在修 PR）。

---

## B. 方案分析

### B1. 仓内先例：`Timeline` 的 `disabled` 标记

```rust
pub struct Timeline {
    pub(crate) timeline: *mut sys::ISteamTimeline,
    pub(crate) disabled: bool,   // 接口不可用标记
    pub(crate) _inner: Arc<Inner>,
}
impl Timeline {
    pub fn set_timeline_game_mode(&self, mode: TimelineGameMode) {
        if self.disabled { return; }          // 方法入口统一守卫
        unsafe { sys::SteamAPI_ISteamTimeline_SetTimelineGameMode(...) }
    }
}
```
每个方法先守卫再解引用，**零公开签名变更**，release 同样安全。

### B2. 影响面分层：23 处断言 ≠ 23 个签名

- **16 个 `Client` getter（lib.rs:349-545，timeline 除外）**：加 `unavailable` 字段 + 守卫，最高频入口；timeline 是仓内唯一已用运行时判空的 getter。
- **6 个 GameServer getter（server.rs:537-596）**：GS 初始化失败路径更常见，同样处理。
- **改动落在接口结构体内部**：构造时设标记、方法入口守卫。只有改成 `Option<Interface>`/`Result<Interface>` 才破坏签名。

### B3. 方案对比

| 方案 | API 破坏 | release 安全 | 成本 | 与仓内风格 |
|------|---------|-------------|------|-----------|
| `unavailable` 标记（Timeline 先例） | 无 | ✅ | 低（21 处构造 + 方法守卫） | ✅ 一致 |
| `Option<Interface>` | 是（全部调用方改 `if let`） | ✅ | 中 | ⚠️ 不一致 |
| `Result<Interface>` | 是（`unwrap` 改 `?`） | ✅ | 高 | ⚠️ 破坏 15+ 签名 |
| 升级 `assert!` | 无 | ❌ release 仍 panic | 低 | ❌ |

**推荐 `unavailable` 标记**。

---

## C. 补充发现

1. **两层防护缺口**：接口结构体自身方法**无任何判空**（如 `friends.rs` 直接用持有的指针），构造时 null 则每个方法都是独立 UB 点。只改 getter 不够，**方法入口必须也加守卫**。
2. **与 #5/#6 叠加**：`init()` 失败后继续调用 → 判空逃逸后 `CString` panic 掩盖真正根因（初始化失败）。守卫应返回**有区分度的值**（空串/错误码），不能静默无信号。

---

## 修复骨架

```rust
// src/lib.rs
pub fn friends(&self) -> Friends {
    unsafe {
        let friends = sys::SteamAPI_SteamFriends_v018();
        debug_assert!(!friends.is_null());   // 可保留作 debug 提示
        Friends { friends, unavailable: friends.is_null(), inner: self.inner.clone() }
    }
}

// src/friends.rs
impl Friends<'_> {
    #[inline]
    fn guard(&self) -> bool { !self.unavailable }

    pub fn get_persona_name(&self) -> String {
        if !self.guard() { return String::new(); }
        unsafe { /* 正常路径 */ }
    }
}
```

---

## 下一步

1. ~~C1 重复检索~~（已完成）
2. ~~server.rs 6 处确认~~（已完成，全为 getter）
3. 提 PR 前做 A5：`gh pr list --search "null guard OR disabled"` 确认无在修 PR

---

## 实施记录（2026-10-10）

**分支** `fix/null-guard-unavailable` @ `15c0108`（基点 `6e4a2bc`，工作树 `wt-fix-null`，未推送）。
**形状**：23 个 `pub(crate) unavailable: bool` 字段 + 258 处方法入口守卫 + getter 字面量 22 处（lib.rs 16 / server.rs 6）+ 子句柄构造点传播；**+1037 / −0，20 个文件，零公开签名变化**。

### 与草稿的偏差（以实测为准）
1. **字段名用 `unavailable`**（getter 处语义即 `ptr.is_null()`，直读），草稿散文中的 `disabled` 仅路线图行出现过。
2. **脚本化批量**：`add_null_guards.py` v3 一次落 253 守卫（返回类型→零值映射全覆盖，0 未知），手工只补脚本到不了的 5 处：
   `io::Write::write` / `io::Read::read` / `io::Seek::seek`（trait 方法非 `pub fn`，正则不抓）；
   `SteamFileWriter::drop` / `NetPollGroup::drop`（**Drop 不是方法**——unavailable 实例会把空指针递给
   `FileWriteStreamClose` / `DestroyPollGroup`，这是草稿没列出的第三层 UB 面）。
3. **`NetworkingMessage::drop` 自带 `!is_null()` 守卫**，核实后不改。
4. **零值语义定案**：`Result→Err(SteamError::IOFailure)`；`NetworkingAvailabilityError::Unknown`（其文档串本就是
   "probably not initialized yet"）；`FriendState::Offline`；`UpdateStatus::Invalid`；`MessageError::NullBuffer`；
   io 侧 `read→Ok(0)` / `write→Err(Other)` / `seek→Err(InvalidInput→Other)`。
5. **`seek` 负偏移绕过 `size-offset==0` 短路**：零实例上 `SeekFrom::Current(-5)` 把 `offset` 绕成 `usize::MAX`，
   release 下随后的 `read` 会解空指针 ⇒ `seek` 一并加守卫；HEAD 上真实实例同样能触发 usize 下溢 panic，
   属既有缺陷、不在本 PR 修（记档）。

### 门禁（同尺对照 HEAD `6e4a2bc`，共享 CARGO_TARGET_DIR，`CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`）
- build --all-targets：0 error。
- test --all-targets：**10 通过 / 11 失败 / 2 忽略** vs HEAD 7/11/2 —— +3 全为新增离线测试
  `unavailable_guard_tests`（零实例断言零值 / io trait 守卫 / drop 不碰空指针）；11 个失败数不变（需 Steam 客户端的既有用例）。
- clippy --all-targets：修复树与 HEAD 输出留档逐行相同（多集 diff 为零）。首跑 `grep -c` 得 122 vs 121，
  复跑对账归零（编译冷热瞬态，非真实 lint）。
- fmt --check：清（守卫行先 `cargo fmt` 归一成多行）。

### 过程留痕
- 脚本 v2 曾有**分隔符吞噬 bug**（构造点正则吃掉字段 `:`/`,` 不回吐 → 约 10 个构造字面量烂掉），v3 修复后编译一次全绿。
- 工具坑照旧：`rg -o '-> …'` 报 unrecognized flag（模式以 `-` 开头）⇒ 文本分析全走 python；
  Mimosa 拦 `open(...,'w')` 动态路径写盘 ⇒ 落盘一律 `Path.write_text`。
- **验证边界**：本机无 Steam 管道 ⇒ 证据 = 在树单测 + 四门禁 + 结构检视，**不是端到端**；
  Mimosa 完整扫描未跑（hook 报 scanner_enobufs），不宣称 safe。
