# 缺陷 #6 深度分析：UGC builder 链式 panic——panic 面属实，但「已写入 Steam 无法回滚」的归因有误

> 依据核查清单完成：A1 复现 ✓ / A2 计数可复算 ✓ / A3 查定义 ✓ / A5 上游时效 ✓ / B1 仓内先例 ✓ / C1 重复检索 ✓
> 行号对照工作区（见主文档头部基准声明）。

## 结论

**缺陷成立，严重度维持「中」，但核心归因需要修正**：

1. **panic 面属实且比主文档描述的更广**：`UpdateHandle` 的 13 个 builder 方法共 **24 个 panic 点**（11 个 `unwrap()` + 13 个 `assert!`），另有 `tags` 经 `SteamParamStringArray::new` 的 `expect()` 间接 panic——**panic 点实为 25**。
2. **主文档的「前序状态已写入 Steam 无法回滚」归因有误**（第 12 处自我更正）：Valve 头文件 `isteamugc.h:296` 明确 `Set*` 是**暂存**（"Set changed properties **before commiting** update with CommitItemUpdate()"），只有 `SubmitItemUpdate` 才提交。链式 panic 时 `submit` 从未被调用 → **工作坊物品的公开状态没有发生任何变更，不存在需要回滚的东西**。复现脚本里的「已提交步骤无法回滚」是模拟器自己假设的语义，不是 Steam 的实际语义。
3. **panic 的真实危害**（修正后的定性）：调用方进程崩溃/状态机中断、无法用 `Result` 优雅降级、**放弃的暂存会话无释放 API**（SDK 中无 `ReleaseUnusedItemUpdateHandle`，全 `isteamugc.h` 搜不到任何 abort/release 函数）。

---

## A. 取证过程

### A1. 复现证据

`repro/ugc_builder_panic_test.rs` 实测（本机 Windows）：

```
--- 场景 1：content_path 传不存在的路径 ---
thread 'main' panicked at ugc_builder_panic_test.rs:50:45:
called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound, ... }
  PANIC —— 链式调用中断

--- 场景 2：description 含内嵌 NUL ---
thread 'main' panicked at ugc_builder_panic_test.rs:43:36:
called `Result::unwrap()` on an `Err` value: NulError(4, [100, 101, 115, 99, 0, ...])
  PANIC —— 链式调用中断

--- 场景 3/4：Result 链模式，同样输入 ---
  返回 Err(路径不可用: ...) / Err(title 含内嵌 NUL) —— 调用方可处理
```

**复现证明了什么**：panic 传播真实、`Result` 模式可优雅降级。
**复现没证明什么**：「Steam 侧状态已写入无法回滚」——那是模拟器里 `committed_steps` Vec 的行为，是**我写进模拟器的假设**，不是 Steam SDK 的行为。见 A3。

### A2. 计数可复算

```bash
# UpdateHandle impl 块内的 panic 点（工作区 src/ugc.rs:927-1128）
awk '/^impl UpdateHandle/,/^\/\/\/ A handle to watch/' src/ugc.rs | grep -o "unwrap()\|assert!" | sort | uniq -c
#   13 assert!   ← Steam API 返回 false 即 panic
#   11 unwrap()  ← CString/canonicalize 失败即 panic
# 合计 24
```

| 口径 | 数值 | 说明 |
|------|------|------|
| builder 方法数 | 13 | `title`…`remove_all_key_value_tags`（另有 `submit` 不 panic） |
| 直接 panic 点 | 24 | 11 unwrap + 13 assert |
| 间接 panic 点 | +1 | `tags` → `utils.rs:321` `expect("String passed could not be converted...")` |
| **合计** | **25** | |
| `ugc.rs` 全文件 `CString::new(...).unwrap()` | 17 | 含 builder 之外的调用点 |
| `#[must_use]` 覆盖 | **6 / 13** | 另 7 个方法（`visibility`/`tags`/`add_key_value_tag` 等）**丢弃返回值无任何警告** |

对照：缺陷 #5 的 73 处是全仓 panic 点；本节的 25 是 `UpdateHandle` 单一结构体内的密集面。

### A3. 关键语义核查：Set* 是暂存，不是写入（本次深挖的核心修正）

`steamworks-sys/lib/steam/public/steam/isteamugc.h:296`：

```c
virtual UGCUpdateHandle_t StartItemUpdate(...) = 0;
// start an UGC item update. Set changed properties before commiting update with CommitItemUpdate()

virtual bool SetItemTitle(UGCUpdateHandle_t handle, const char *pchTitle) = 0; // ...
STEAM_CALL_RESULT( SubmitItemUpdateResult_t )
virtual SteamAPICall_t SubmitItemUpdate(UGCUpdateHandle_t handle, const char *pchChangeNote) = 0;
// commit update process started with StartItemUpdate()
```

语义链条：`StartItemUpdate` 开会话 → `Set*` **往会话里暂存属性** → `SubmitItemUpdate` 才 commit。

因此链式 panic 时：
- ✅ panic 真实发生、链中断、`submit` 未调用
- ✅ 暂存会话被放弃（**无释放 API**——全头文件无 release/abort/cancel 函数，`k_UGCUpdateHandleInvalid` 只是无效值常量）
- ❌ **「公开物品状态已被改动、无法回滚」不成立**——commit 从未发生

同时按 A3 纪律订正一个量级词：`k_UGCUpdateHandleInvalid` 是 `uint64` 全 1（`isteamugc.h:29`），不是本节相关的计数问题，仅记录避免误用。

### A5. 上游时效性

`git log --oneline -5 -- src/ugc.rs`：近期提交（`c205b3b`/`5149d85`/`8d91251`）全部是 `EResult`/`SteamError` 处理重构，**builder 签名未动**（仍是 `-> Self` + `unwrap/assert`）。缺陷在当前 master 仍在。

### C1. 重复检索

```bash
gh issue list --state all --search "builder panic OR UpdateHandle OR unwrap OR canonicalize"
```

命中 12 条，逐条核对后**无重复报告**：#82/#220 是用户侧 `Client::init().unwrap()`（非 builder）、#115/#295 是 UGC query 用法、#212/#222/#127/#37/#22/#114/#82 无关。

---

## B. 方案分析

### B1. 仓内先例（同一结构体内就有正例）

1. **`submit` 自己用 `.ok()` 处理 CString**（`ugc.rs:1100`）：
   ```rust
   let change_note = change_note.and_then(|v| CString::new(v).ok());
   ```
   同一个 `impl UpdateHandle` 里，`submit` 不 panic、其余 13 个方法全 panic——**风格分裂就在眼皮底下**。
2. **`matchmaking.rs:729` 已有 Result 风格的构造器先例**：
   ```rust
   pub fn try_new(key: &'a str) -> Result<Self, LobbyKeyTooLongError>
   ```
3. 主文档缺陷 #5 已列的 10 处正确处理（`user_stats.rs` 的 `map_err`、`ugc.rs:1100` 的 `.ok()`）。

### B2. 影响面分层

| 层 | 数量 | 破坏面 |
|----|------|--------|
| `UpdateHandle` builder 方法 | 13 方法 / 25 panic 点 | 改签名 → 破坏 13 个公开方法 |
| `#[must_use]` 缺口 | 7 方法 | 加属性 → **零破坏**，立即可做 |
| `tags` 间接 panic | `utils.rs:316` | `SteamParamStringArray` 是 `pub(crate)`，改内部 → 零破坏 |

### B3. 方案对比

| 方案 | 修 panic | 破坏 API | 与仓内先例 |
|------|---------|---------|-----------|
| 13 方法 `-> Self` 改 `-> Result<Self, SteamError>` | ✅ | **是**（13 个签名） | ✅ `try_new` 先例；可搭维护者 0.13 的 option 4 破坏窗口 |
| 增设 `try_*` 变体，旧方法保留 | ✅ | 否 | ⚠️ API 面翻倍，双份维护 |
| 仅补 `#[must_use]` + `# Panics` 文档 | ❌ 只改善可见性 | 否 | 可作过渡 |
| 全部改「失败跳过继续」 | ❌ 静默吞错 | 否 | ❌ 比 panic 更糟 |

**推荐**：方案 1 搭 0.13 破坏窗口（维护者已在 #253 规划 option 4 破坏性变更，同窗口收编本项可分摊迁移成本）；过渡期先做零破坏的两件事——补 7 处 `#[must_use]`、13 个方法加 `# Panics` 段。

### 对主文档的修正要求（D1/D2）

- 主文档 0.1 节「**前 5 步的资源与状态全部丢失，且前序状态已写入 Steam 无法回滚**」→ 改为「前 5 步的暂存被放弃（SDK 无释放 API），**公开状态未变更**；危害是 panic 中断 + 无法降级」。
- 摘要表 #6 证据列「panic unwind 时前序状态已写入 Steam 无法回滚」→ 同步。
- 复现脚本 `ugc_builder_panic_test.rs` 场景 1/2 的提示文案含同一错误归因，需注释标明「模拟假设，Steam 实际为暂存语义」。

---

## 深挖产出的新事实

| # | 新事实 | 证据 | 影响 |
|---|--------|------|------|
| 1 | **Set* 是暂存语义，commit 只在 submit** | `isteamugc.h:296/320` | 主文档「无法回滚」归因错误（第 12 处更正） |
| 2 | panic 点 25（24 直接 + 1 tags 间接），非泛指「链式 panic」 | awk+grep 可复算 | 计数精度 |
| 3 | 同 impl 内 `submit` 用 `.ok()`、其余全 panic | `ugc.rs:1100` vs builder | B1 强先例：风格分裂在同一结构体内 |
| 4 | `#[must_use]` 只覆盖 6/13，7 个方法丢弃无警告 | grep -B1 统计 | 零破坏可立即修的缺口 |
| 5 | SDK 无 handle 释放 API | `isteamugc.h` 全文无 release/abort | 放弃会话只能靠 Steam 客户端回收 |
| 6 | 复现脚本自身携带错误归因 | 场景 1/2 输出文案 | 脚本需加注释，否则误导后续读者 |

**严重度维持「中」**：panic 面真实且密集（25 处），但触发需特定输入（NUL/不存在路径/Steam 返回 false），且无数据损坏后果（暂存未提交）。

---

## 实施记录（2026-10-10）

**落地**：分支 `fix/ugc-builder-result` @ `f54fb6c`（基点 `6e4a2bc`，未推送），4 文件 +466/−213。

**范围**：按 B3 方案 1 直做（本仓自己的分支无迁移负担，过渡期的 `#[must_use]`/`# Panics` 补丁只对「不能动签名」的场景有意义，此处跳过）——22 个 builder 方法 `-> Self` 改 `-> Result<Self, SteamError>`（13 UpdateHandle + 9 QueryHandle，名单在提交 body）；错误映射钉死为 NUL→`InvalidParameter` / canonicalize NotFound→`FileNotFound` 其余 io→`IOFailure` / sys `Set*` 假→`InvalidParameter`。三个「Panics if」文档段换成 `# Errors` 段；`#[must_use]` 从 6 个被转方法摘除（Result 本身隐含 must_use），`start_item_update` 保留。骑缝项：`init_for_game_server` 保 `-> bool`（NUL→false），避免级联 `Server::init_for_game_server` 的公开签名。`SteamParamStringArray::new` → `Option<SteamParamStringArray>`（失败路径回收已转换的 CString，不泄漏）。行为改善一条：QueryHandle 串方法原 release 下 `debug_assert!(ok)` 编译掉即静默失败，现在回 `Err`。出范围两条记档：`QueryHandle::fetch` 的 `handle.unwrap()`（fetch-consumed 语义，另一类缺陷）、`submit` 的 change_note `.ok()` 静默丢弃（无 panic，记为已知降级）。

**实施途中抓到的三个自伤**（都不是引擎语义问题，是补丁脚本与测试设计问题）：

1. **conv 收集顺序 ≠ 源码顺序**：collector 式提取按正则类别（先 CString 后 canonicalize）输出转换行，把 `content_path`/`preview_path` 的两行倒置——canonicalize 的结果本应**喂给** CString 转换（base 是先 `path.canonicalize().unwrap()` 再 `to_string_lossy()`），倒置后 canonicalize 结果成为未使用变量（build 警告暴露），且上传路径从规范化路径退化为原始路径（语义漂移，无警告）。教训：**多类 conv 共存时必须按源码行序输出**，build 警告清零是对账手段（base 0 警告 ⇒ 修后也必须 0）。
2. **签名提取正则吃掉泛型**：`(?:<.*?>)?` 把 `tags<S: AsRef<str>>` 的泛型参数消费掉没拼回，`pub fn tags(self, tags: Vec<S>, ...)` 编译报 E0425。单泛型方法一个，手工补回。
3. **离线测试踩 `QueryHandle::drop` 段错误**：测试用 null `ISteamUGC` 指针构造句柄，错误路径全部先于 sys 调用返回——设计上安全；但 builder 方法**消费 `self`**，返回 `Err` 时 `self` 立即析构，`QueryHandle::drop`（ugc.rs:1217）里 `ReleaseQueryUGCRequest(self.ugc, handle)` 拿空指针直接 0xc0000005。症状有迷惑性：并行首跑崩在 `update_key_value_tag_nul`（UpdateHandle 无 Drop，实为并行线程归属错报）、串行崩在 `query_language`、单独跑 `update_key_value` 通过——三轮才钉到「**崩的从来不是被测方法体，是 Drop**」。修法：测试 helper 用 `handle: None`（Drop 有 `if let Some` 守卫），若回归让错误路径漏到 sys 调用，`handle.unwrap()` 会 panic 而非段错误。**教训：给带 Drop 的句柄类型做离线测试，句柄槽位必须选「Drop 短路」的值，而不是选「无副作用」的真值**。

**门禁（同尺）**：fmt 0 / build 0（0 警告，含 conv 顺序修复后）/ clippy kind 多重集对 base `6e4a2bc` **0 新增 1 消失**（消失的是 `into_iter` lint，正打在旧 `SteamParamStringArray::new` 的 `vec.into_iter()` 上，重写为 `for` 循环后自然消掉，净改善）/ 串行 test **15 通过 11 失败 2 忽略** = 基线 7-11-2 + 8 个新离线测试，**11 个失败名单与 base 逐名相同**（全为本机 NoSteamClient 环境性）/ 并行 10×0 段错误（修 QueryHandle drop 问题后）。

**待办移交**：上游 PR 须等用户拍板（五个分支都在等）；提 PR 前先跑 A5 `gh pr list --search` 查重。
