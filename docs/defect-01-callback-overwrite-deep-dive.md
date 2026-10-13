# 缺陷 #1 深度分析：同类型回调重复注册静默覆盖

> 依据核查清单完成：A1 复现 ✓ / A2 计数可复算 ✓ / A5 上游时效 ✓ / B1 仓内先例 ✓ / C1 重复检索 ✓

## 结论

**缺陷成立，严重度：高。** 且本次深挖发现两个主文档未覆盖的复合缺陷：

1. **陈旧 Handle 误杀新注册**——`CallbackHandle::drop` 按 ID 删除，不校验归属。
2. **内部自动注册与用户注册共用同一张表**——用户注册 `NetConnectionStatusChanged` 会静默杀掉 NetworkingSockets 的内部连接状态处理，反之亦然。

两者的根因相同：**回调表以回调类型 ID 为唯一键（`HashMap<i32, Box<dyn FnMut>>`），没有「注册实例」概念**。

---

## A. 取证过程

### A1. 复现证据

`repro/callback_overwrite_test.rs` 镜像 `callback.rs:192-210`（register）与 `callback.rs:162-175`（Handle drop）的精确模式，实测：

```
--- 场景 A：同一类型注册两个回调 ---
  派发事件 →
  [回调2] 收到: 事件X
  实际: 只有回调2 收到，回调1 被静默覆盖（无警告、无文档）

--- 场景 B：旧 Handle 在新注册之后被 drop ---
  drop(回调1的Handle) 之后派发事件 →
  实际: 回调2 被误删 —— remove(id) 不校验归属，按 ID 杀掉了别人的注册
```

源码锚点：

```rust
// callback.rs:198 —— 覆盖点
inner.callbacks.callbacks.lock().unwrap().insert(
    C::ID,                                    // 键 = 回调类型 ID，非注册实例
    Box::new(move |param| { ... }),
);

// callback.rs:167 —— 误杀点
cb.remove(&self.id);                          // 按 ID 删除，不比对是不是自己注册的那个
```

### A2. 计数可复算

| 口径 | 数值 | 命令 |
|------|------|------|
| `Callback` trait 实现类型 | 40 | `grep -rn "impl_callback!" src/ \| wc -l` |
| `process_callbacks` 公开可达 | 38 | `grep -c "Self::" src/callback.rs` |
| 公开 `register_callback` 入口 | 2 | `lib.rs:345`（Client）、`server.rs:205`（GameServer） |
| 仓内自动注册调用点 | 4 | `networking_sockets_callback.rs:27`、`networking_utils.rs:108`、`networking_messages.rs:188/209` |
| 文档声明「替换语义」的位置 | 3 | `networking_messages.rs:160/203`、`networking_utils.rs:102` |

**调用面解读**：任意 38 种可注册回调类型 × (用户二次注册 + 内部自动注册) 都是潜在碰撞点。

### A5. 上游时效性

当前源码 `callback.rs:198` 确认仍是 `insert`，无上游修复。C1 检索（见下）也无相关 Issue/PR。

---

## B. 方案分析

### B1. 仓内先例盘点（本节是本次深挖最重要的修正）

覆盖语义在仓内**部分是刻意契约**，三处方法文档白纸黑字写着：

```rust
// networking_messages.rs:160, 203 与 networking_utils.rs:102
/// Calling this function more than once will replace the previous callback.
```

这改变了责任划分：

- **可以说的**：`Client::register_callback`（`lib.rs:340-346`）本身**没有任何文档**说明覆盖语义——文档只讲线程归属（lib.rs:330-339）。用户在通用入口遇到静默覆盖是文档缺口。
- **不能说的**：「替换语义本身是缺陷」——仓内 3 处已把它文档化为设计意图。正确主张是**语义不一致**：包装方法声明了替换，通用入口没声明却是同样行为。

### 新发现 1：陈旧 Handle 误杀（超出「覆盖」范畴）

场景 B 展示的行为与覆盖不同：**后注册的回调存活期间，先注册者句柄的 drop 会把它删掉**。这不是覆盖，是删除语义的 ID 错配。

加重因素：`networking_utils.rs:108` 与 `networking_messages.rs:188/209` 用 `std::mem::forget(call_handle)` 永久泄漏句柄来规避 drop——说明仓内作者**已意识到 Handle 生命周期有问题**，但只在这三处绕过，通用路径 `lib.rs:782` 等仍是正常 drop。

### 新发现 2：内部注册 vs 用户注册互杀

`networking_sockets_callback.rs:27` 在首次建立 socket 连接时自动注册 `NetConnectionStatusChanged`：

```rust
let callback = unsafe {
    register_callback(&inner, move |event: NetConnectionStatusChanged| {
        handler.callback(event);
    })
};
```

而 `NetConnectionStatusChanged` 是公开 Callback 类型（在 `CallbackResult` 枚举中，`callback.rs:57`）。后果：

- 用户先注册 → 内部注册时把用户的杀掉（用户监听静默失效）
- 内部先注册 → 用户注册时把内部的杀掉（**NetworkingSockets 连接状态处理静默失效**，表现为 socket 断线不回调，极难排查）

两个方向都是静默的，无日志无警告。

### 方案对比

| 方案 | 修复覆盖 | 修误杀 | 修内互杀 | API 破坏 | 成本 |
|------|---------|-------|---------|---------|------|
| `Vec<Arc<dyn FnMut>>` 按 ID 存多值 + Handle 持注册序号 | ✅ | ✅ | ✅（多值共存） | 无（Handle 结构私有） | 中 |
| 仅在 `register_callback` 文档声明覆盖语义 | ❌ 行为不变 | ❌ | ❌ | 无 | 极低 |
| `register_callback` 改返回 `Result` 拒绝重复注册 | ✅ 拒绝而非覆盖 | ❌ | ✅ | **有**（签名变更） | 低 |

**推荐方案 1**——与主文档决策 1（`Dispatcher` 用 `Vec<Arc<CallbackFn>>`）一致；Handle 内部加单调递增序号即可修误杀，序号字段是私有的，零公开签名破坏。方案 2 可作为方案 1 未合并前的临时缓解。

---

## C1. 重复检索

```bash
gh issue list --repo Noxime/steamworks-rs --state all \
  --search "register_callback overwrite OR multiple callbacks OR replace OR duplicate"
gh issue list --repo Noxime/steamworks-rs --state all --search "CallbackHandle drop remove"
```

结果 4 条（#6/#101/#22/#253）全部无关：#22 是 async 回调需求、#253 是死锁、#6 是接口缺失、#101 是使用求助。**无重复报告**。

---

## 深挖产出的新事实

| # | 新事实 | 证据 | 影响 |
|---|--------|------|------|
| 1 | Handle drop 按 ID 误杀新注册 | `callback.rs:167` + 复现场景 B | 缺陷面比「覆盖」更大 |
| 2 | 仓内 3 处已文档化替换语义 | `networking_messages.rs:160/203`、`networking_utils.rs:102` | 责任主张须改为「文档不一致」而非「语义错误」 |
| 3 | 内部自动注册与用户注册同表互杀 | `networking_sockets_callback.rs:27` vs 公开 Callback 类型 | 新增静默失效路径，socket 断线不回调 |
| 4 | 仓内用 `mem::forget(handle)` 绕过 drop 问题 | `networking_utils.rs:108`、`networking_messages.rs:188/209` | 作者已知 Handle 生命周期有问题，仅局部规避 |
| 5 | 覆盖时旧 Box 被 drop（闭包资源释放）| `HashMap::insert` 语义 | 无额外危害，但旧闭包持有的状态被销毁 |

**严重度结论不变：高。** 静默失效 + 无文档 + 内部互杀叠加，且复现零门槛（无需 Steam 客户端参与逻辑层）。


## 实施记录（2026-10-10，分支 `fix/callback-multi-registration` @ `5d2bcc9`，基点 `6e4a2bc`，未推送）

**落地形态**：6 文件 +277/−42（lib.rs / callback.rs / server.rs / networking_messages.rs / networking_utils.rs / utils.rs）。即本档 B 节方案 1，零公开签名破坏（CallbackHandle 字段私有）：

- **表**：`HashMap<i32, Vec<(u64, Box<dyn FnMut(*mut c_void) + Send)>>`，键 = 回调类型 ID，值 = 按注册序排列的 (seq, handler)。同类型多注册共存、按注册序派发。
- **Handle 修误杀**：`CallbackHandle` 增持自己注册的 seq（per-Context `AtomicU64`）；Drop 按 seq `retain`（表项空则整键清除）。陈旧 Handle 的 seq 已不在表内 ⇒ 什么都不删。
- **派发**：两处闭包（run_callbacks / process_callbacks）抽成 `Callbacks::dispatch`，锁内调用维持原状——**回调内再注册的死锁类不变，归 #2 残余**（已在 dispatch 文档注明），本档范围只覆盖覆盖/误杀/内互杀三缺陷。
- **替换语义保真**：新 `register_replacing_callback` 助手 = 注册 + 把句柄存进 `Callbacks.replacing: Mutex<HashMap<i32, CallbackHandle>>` + 重注册时 drop 旧句柄（**先放注册表锁再 drop**，因 drop 取表锁，防锁序环）。5 个 `mem::forget` 站点归宿：messages ×2（session_request/session_failed，文档化替换）与 utils relay ×1（同）走助手保真；utils gamepad ×2（原隐式覆盖，last-wins）走同一助手保真。句柄泄漏随之消除。
- **sockets 内部注册（get_or_create_connection_callback）零改动**：Weak 缓存 + 新 Drop 语义下，句柄最后一份 Arc 消亡即自移除自身 seq ⇒ 重注册自然成立；与用户注册共存 = 多值表免费赠品（新发现 2 直接修复）。

**测试**（lib.rs `callback_multi_registration_tests`，5 条，离线、TestCallback 用界外 ID 9_999_999、直接驱动生产 `Callbacks::dispatch`）：复现场景 A（双注册均触发且按序）/ 场景 B（先注册句柄 drop 不误杀后者）/ 末句柄清表 / 内部+用户共存（`get_or_create_connection_callback` 后计数 2，drop 内部句柄后余 1）/ 替换助手只换自己的（用户注册存活、被换者不触发）。测试 helper 沿用 `std::mem::forget(Arc::clone(&inner))`（#5 会话钉死的 Manager::drop→SteamAPI_Shutdown 崩溃机制）。

**门禁（同尺 vs HEAD 6e4a2bc，共享 target dir，DEBUG=0 INCREMENTAL=0）**：fmt --check exit 0；build exit 0；clippy kind-diff **0/0**；串行 test **12 通过 / 11 失败 / 2 忽略** = 基线 7-11-2 + 恰好 5 条新测（11 失败全为本机 NoSteamClient 环境性，两树同集）；并行 10×0 segv。

**与 #2 残余的关系**：本档表方向已落地（defect-02 讨论的「表存 Box 需放回 or 改 Arc」中的表侧前提）；但「回调内 register/反注册死锁」未动——dispatch 锁内调用与基线逐字同行为。
