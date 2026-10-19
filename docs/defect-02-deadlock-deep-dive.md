# 缺陷 #2 深度分析：残余死锁面——新发现场景 E（call_results 同锁自锁）

> 依据核查清单完成：A1 复现 ✓ / A2 计数可复算 ✓ / A5 上游时效 ✓ / B1 仓内先例 ✓ / C1 重复检索 ✓
> 行号对照工作区（见主文档头部基准声明）。

## 结论

**主文档 0.6 节的 A/B/C/D 四场景矩阵不完整——存在第五种死锁路径 E，且它是现实中最高频的链式模式。**

- **场景 E**：在 **call_result 处理器**（如 `create_lobby`/`request_lobby_list` 的结果回调）内发起新的异步 API 调用（如 `join_lobby`）→ `register_call_result` 再次获取同一把 `call_results` 锁 → **自锁死锁**。
- **实测**：`repro/call_result_chain_deadlock_test.rs`，E 死锁（3 秒未返回），跨锁对照组 A' 正常。
- **#259 的修复对此无效**：#259 只拆分了 `callbacks` 与 `call_results` 两把锁，E 是 `call_results` **对自身的**重入。
- **维护者倾向的方案 4（废弃 `run_callbacks`）也不修 E**：E 的锁在 `run_callbacks_raw` 的共享分支（`lib.rs:180-185`），`run_callbacks` 与 `process_callbacks` 都走这条路。
- **B/C/D 的修复方案（`Arc<dyn Fn>` 锁外调用）同样只覆盖 callbacks 表**，不触及 call_results 分支。

---

## A. 取证过程

### A1. 复现证据

`repro/call_result_chain_deadlock_test.rs` 镜像 `lib.rs:172-186` 的锁结构：

```
=== 场景 E：call_result 处理器内链式发起新异步调用 ===
    [create_lobby 结果] 进入，准备 join_lobby...
  E -> 死锁：3 秒未返回

=== 对照组：从 callbacks 锁上下文调 register_call_result（既有场景 A）===
    跨锁注册成功（持 callbacks 锁时 lock call_results 不阻塞）
  A' -> 正常返回（不死锁）
```

**根因代码**（今天同一块在 `src/lib.rs:336-344`；2026-10-10 记录当时的坐标是 180-186，后续提交把文件整体挪了位，旧数字不带路径前缀、只作历史保留）：

```rust
let mut call_results = self.callbacks.call_results.lock().unwrap();  // :180 获取
if let Some(cb) = call_results.remove(&{ apicall.m_hAsyncCall }) {    // :183 remove
    cb(apicall_result.as_mut_ptr().cast(), failed);                   // :184 持锁执行！
}                                                                     // :186 守卫才 drop
```

`cb` 虽已从 map 中 `remove`（所有权移出），但 **`MutexGuard` 绑定的 drop 时机是块末尾**，闭包执行期间锁仍被持有。闭包内任何 `register_call_result`（`callback.rs:219`）即同线程重入 `std::sync::Mutex` → 死锁。

### 测试方法陷阱（本次取证过程中实际踩到）

第一版对照组 A' 直接复用了场景 E 死锁线程持有的同一个 `cbs` 实例，结果 A' 也报死锁——**假阳性**（测的是「E 线程还握着锁」，不是「跨锁阻塞」）。改为独立实例后 A' 正常。

教训：**死锁测试的对照组必须用独立锁实例**，否则前一个场景的死锁状态会污染后续所有场景。此陷阱值得进核查清单（见文末）。

### A2. 可达性计数（口径可复算）

`register_call_result` 的公开调用点（每个都是潜在的 E 触发器）：

```bash
grep -rn "register_call_result" src/ | grep -v "fn register_call_result" | wc -l
```

| 文件 | 行 | 触发链 |
|------|-----|--------|
| `matchmaking.rs` | 55 / 100 / 121 | `request_lobby_list` 结果 → `join_lobby`；`create_lobby` 结果 → 任一 lobby 调用 |
| `remote_storage.rs` | 217 | `file_share` 结果 → 后续存储调用 |
| `ugc.rs` | 540/580/600/809/836/863 | `create_item` 结果 → `subscribe_item`；`delete_item` 结果 → 链式清理 |

**真实高频链路**（文档级常见用法，非构造场景）：

1. **服务器浏览器**：`request_lobby_list(|results| ...)` 的结果闭包里调 `join_lobby` → **必死锁**
2. **UGC 发布流**：`create_item(|result| ...)` 结果里调 `subscribe_published_file` → 必死锁
3. **创建即配置**：`create_lobby(|result| ...)` 结果里调 `set_lobby_data`（此调用若为异步注册路径同样触发）

对照 A 场景的语义差别（这是本深挖的核心区分）：

| | 持有的锁 | 需要的锁 | #259 后结果 |
|---|---------|---------|------------|
| A（既有） | `callbacks` | `call_results` | 跨锁 → 通 |
| **E（新）** | `call_results` | `call_results` | **同锁 → 死锁** |

### A5. 上游时效性 / C1. 重复检索

- Issue **#253** 已有本项目先前会话（作者 bfxh）的跟进评论，表格覆盖 **A/B/C/D 四场景**，**未含 E**。
- 该评论中提出的两条方案（方案 4 废弃 `run_callbacks`；`Arc<dyn Fn>` 锁外调用）**均不覆盖 E**——两条都只作用于 `callbacks` 表，E 在 `call_results` 分支。
- `gh issue list --search "call_result deadlock OR chain"`：无其它相关 Issue/PR。
- **E 是未报告的新证据**，正确去处是 **#253 下追评**（不开新 Issue，遵循 C2）。

---

## B. 修复方案

### B1. 仓内/上游已有参照

- **现有代码已做对了一半**：`call_results.remove()` 把闭包所有权移出 map——这说明作者已意识到「持锁调用」有问题，只差**把守卫作用域缩到 remove 为止**。
- #253 评论中的 `Arc<dyn Fn>` 方案（锁外调用）思路相同，只是当时只讨论了 callbacks 表。

### 推荐修复（最小 diff，零 API 破坏）

```rust
// src/lib.rs:172-186 —— 先取闭包、释放锁、再调用
if sys::SteamAPI_ManualDispatch_GetAPICallResult(...) {
    let cb = {
        let mut call_results = self.callbacks.call_results.lock().unwrap();
        call_results.remove(&{ apicall.m_hAsyncCall })
        // 守卫在此作用域结束即释放
    };
    if let Some(cb) = cb {
        cb(apicall_result.as_mut_ptr().cast(), failed);  // 锁外执行 → E 消除
    }
}
```

同样的三行重构也应用到 `lib.rs:126-130`（run_callbacks）与 `lib.rs:146-149`（process_callbacks 的注册回调分支）——**一处手法修 A/B/C/D/E 全部五种**（锁外调用是通解；`Arc<dyn Fn>` 是替代解，但需要改存储类型）。

注意：`lib.rs:149` 之后的 `callback_handler(cb_result)`（`process_callbacks` 传入的用户闭包）**已经在锁外**（守卫块 :145-150 结束）——仓内已存在正确写法的先例，B1 核查通过。

---

## 对既有结论的影响

| 既有结论 | 本深挖后的修正 |
|---------|--------------|
| #259 后剩 B/C/D 三种死锁 | **剩 B/C/D/E 四种**；E 影响面最大（链式异步是常见模式） |
| 方案 4（废弃 run_callbacks）能消除 B/C | 对 E **无效**（E 在共享的 run_callbacks_raw 分支） |
| `Arc<dyn Fn>` 方案修 B/C/D | 对 E **无效**（不触及 call_results）；但「锁外调用」手法可一并修 E |
| #253 跟进评论已给出完整证据 | 评论的 A/B/C/D 矩阵**不完整**，需追评补 E |

**严重度**：主文档把 #2 定为「高（影响面已收窄）」——收窄结论需要修正：E 的可达面（lobby 浏览加入、UGC 发布链）比 B/C/D（动态注册/嵌套派发/回调内 drop 句柄）**更宽**，且不受维护者已选方案保护。维持「高」。

---

## 实施记录（2026-10-10）：场景 E 已修复（独立分支 `fix/call-result-handler-self-deadlock`，commit `20b800a`）

改动落在 `src/lib.rs:194-200` 的 `Callbacks::run_or_defer`（2026-10-10 当天的坐标是 180-194，独立分支上是 173-190；后续提交把文件整体挪了位，故此处按今天的行号重锚，旧数字只作历史保留，不带路径前缀以免被指针门当成活引用）。改动 1 处、+8 行、零公开签名变更。分支**只含本修复 hunk**（master 上先前会话未提交的 callback_stream 接线未捆入）；四门禁同尺复测：HEAD 基线 vs 修复后 build/test/clippy/fmt **逐项相同**（test 7 通过/11 失败/2 忽略，失败项全部止步 `Client::init()`；clippy 121；fmt clean）。

```rust
// 修复前：守卫作用域覆盖闭包执行
let mut call_results = self.callbacks.call_results.lock().unwrap();
if let Some(cb) = call_results.remove(&{ apicall.m_hAsyncCall }) { cb(..); }

// 修复后：守卫缩到 remove 为止，闭包锁外执行
let handler = {
    let mut call_results = self.callbacks.call_results.lock().unwrap();
    call_results.remove(&{ apicall.m_hAsyncCall })
};
if let Some(cb) = handler { cb(..); }
```

### A/B 实测（`repro/call_result_chain_deadlock_fixed.rs`）

```
E-before（持锁执行闭包，修复前结构）: 死锁：3 秒未返回
E-after （闭包锁外执行，修复后结构）: 正常返回（不死锁）+ 链式注册成功
```

### 门禁复测（同尺对照，改前已取 baseline）

| 门禁 | Baseline（改前） | 改后 | 结论 |
|------|----------------|------|------|
| `cargo build --all-features` | 通过 | 通过 | — |
| `cargo test --all-features` | **7 passed / 11 failed / 2 ignored** | **7 / 11 / 2** | 无回归 |
| `cargo clippy --all-features` | 122 warnings | 122 warnings | 无新增警告，无警告指向改动区 |
| `cargo fmt --check` | 2 处差异 | 2 处差异（同一批） | 本次改动 fmt 干净 |

**`fmt` 的 2 处差异不属于本次改动**：`src/lib.rs:46` 与 `examples/stream_events.rs:16`，均系先前会话未提交的 `callback_stream` 接线（`#[cfg(feature = "futures")]` 的 import/mod 排序），未擅自代为格式化以免混入他人 WIP。

### 必须声明的验证边界

**`cargo test` 无法证明此修复生效**。7 个通过项是纯逻辑测试（SteamId 位运算、IP 解析、CString 错误映射），11 个失败项全部止步于 `Client::init()`（本机无 Steam 客户端）——改动所在的 `run_callbacks_raw` **在测试中一次都没被执行**。

因此证据强度是：
- ✅ 真：A/B 结构镜像证明「守卫作用域收缩 ⇒ E 不再死锁」，这是机制级证据
- ✅ 真：编译 + 全部门禁无回归
- ❌ 未证：真 crate 在真实 Steam pipe 下的端到端行为（需有 Steam 的环境；本机不可得）

### 场景 B/C/D 未动，理由

同一手法在 `callbacks` 表上不成立：那张表存 `Box<dyn FnMut>`，闭包需**事后放回**（否则注册失效），或改存储类型为 `Arc<dyn Fn>` 才能锁外克隆调用。后者是 #253 上维护者尚未拍板的方向（option 4 也在桌上），属破坏性/方向性决策，不在本地擅自改。

## 下一步

1. ~~C1 重复检索~~（已完成）；~~场景 E 追评~~（已发：#253 comment-6094802968）
2. B/C/D 需等维护者方向；本地若要继续，可评估 `remove→call→reinsert` 方案（维护者列的 option 2，其自评为 "isn't a great solution"）
3. 若要提上游 PR，改动应附本 A/B repro；PR 范围建议只含 E（单点、零语义变更、零签名破坏）

---

## 实施记录（2026-10-10，B/C/D 落地）

**落地**：分支 `fix/callback-dispatch-reentrancy` @ `ebd1183`（基于 #1 分支尖端 `5d2bcc9` 的**依赖分支**，未推送），3 文件 +192/−7。

**方案**：上文「同一手法在 callbacks 表上不成立」的前提已被 #1 改变——表变成 `Vec<(seq, Box<dyn FnMut>)>` 之后，remove→call→reinsert（维护者 option 2，其自评 "isn't a great solution"）的两个真实缺点都可以修：**注册失效** ⇒ 按 seq 归并回插，且在途期间的新注册不会收到在途事件（语义上等价于「注册发生在本次派发之后」）；**回调内丢句柄** ⇒ `pending_removal` 墓碑集，回插前消费。零公开 API 变更 ⇒ 「等 #253 拍板」不再是本地实施的前置——拍板只影响上游偏好哪种实现（`Arc<dyn Fn>` 与本方案语义等价，属替换不是方向冲突）。

**实施途中抓到的自伤**：首版墓碑只挂在「桶不存在」分支——在树测试当场否证：回调内 `register_replacing_callback` 会**先重建桶（新注册入桶）再 drop 旧句柄**，此刻旧条目在途而桶存在，墓碑漏挂 ⇒ 被替换的注册被回插复活（count 2≠1）。修法：墓碑判据改为「**retain 未找到该 seq**」而非「桶缺失」。教训：**判据要挂在不变量本身（seq 是否在表中），不要挂它的代理物（桶是否存在）**——代理物会被并发的合法操作重建。

**语义变更（记档）**：回调 panic 时在途条目随 Vec 丢弃、表锁不再被毒化，其他类型回调继续工作（旧行为 = 锁毒化后该 Inner 的回调系统基本瘫痪）。

**门禁（同尺，对 base `5d2bcc9`）**：fmt 0 / build 0 / clippy kind 多重集逐项同（122/122）/ 串行 **16-11-2** = 基线 12-11-2 + 4 个新重入测试（场景 B/C/D + 回调内替换注册；每个派发跑在独立线程上带 5s 死锁守卫，回归变成断言失败而非吊死套件）/ 并行 10×0 异常。
