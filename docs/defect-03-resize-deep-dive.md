# 缺陷 #3 深度分析：`apicall_result.resize(m_cubParam)` 无上限校验

> 依据核查清单（`CHECKLIST-defect-analysis.md`）完成：A1 复现 ✓ / A2 计数可复算 ✓ / A3 查类型定义 ✓ / A5 上游核查 ✓ / B1 仓内先例 ✓ / C1 重复检索 ✓

## 结论

**缺陷成立但严重度维持「低」**（与主文档一致），且经本次深挖有两处修正：

1. **`m_cubParam` 越过 `i32::MAX` 时，传给 C 的 `cubCallback` 参数静默变负数**——这是比原「无上限校验」更精确的缺陷点，此前文档未指出。
2. **「无上限校验」不是 Rust 绑定特有问题**：Valve 官方手动派发模板（`steam_api.h:189`）同样是 `malloc(pCallback->m_cubParam)` 不设上限。这是 **Valve 契约层面的信任假设**——Rust 绑定忠实地镜像了官方模板，把它算作「Rust 绑定的缺陷」有失准确。

因此本缺陷的准确定性是：**继承自 Valve 模板的信任假设 + 一个 Rust 侧特有的 u32→i32 静默截断**。

---

## A. 取证过程

### A1. 复现证据

`repro/resize_oom_test.rs` 实测输出：

```
resize(512): 5.9µs        ← 典型回调
resize(64KB): 103.1µs     ← 远程存储回调
resize(16MB): 25.07ms     ← 大回调
u32::MAX = 4294967295 (0xffffffff)
as usize (64位) = 4294967295 → 64 位平台无截断，上限约 4GB
```

**新增** `repro/cubparam_cast_test.rs` 实测 u32→i32 转换：

```
m_cubParam = 0x7fffffff (2147483647 B) → as c_int = 2147483647  无溢出
m_cubParam = 0x80000000 (2147483648 B) → as c_int = -2147483648  ⚠ 负数！
m_cubParam = 0xffffffff (4294967295 B) → as c_int = -1           ⚠ 负数！
```

且 **两侧转换口径不一致**：
- `src/lib.rs:323` resize 用 `m_cubParam as usize`（64 位无截断，分配 4GB）
- `src/lib.rs:329` C 调用用 `m_cubParam as _` → `c_int`（>2^31-1 变负）

### A2. 类型定义核查（原清单 A3）

| 项 | 类型 | 证据 | 上限 |
|----|------|------|------|
| `SteamAPICallCompleted_t.m_cubParam` | `uint32` | `isteamutils.h:269`、`windows_bindings.rs:4662` | 4294967295（约 4GB） |
| `GetAPICallResult` 的 `cubCallback` 参数 | `c_int` | `linux_bindings.rs:20284`、`steam_api.h:225` | 2147483647（约 2GB） |
| `CallbackMsg_t.m_cubParam`（外层回调结构） | `c_int` | `windows_bindings.rs:1750` | 2GB |

**原判断修正**：早期分析曾把 `m_cubParam` 猜成可能到达 `usize::MAX` 的值，实际是 `uint32`，上限 4GB——这条更正已在主文档第 0 节记录，本次深挖进一步发现**结构体自身的两个字段（`CallbackMsg_t.m_cubParam` 是 c_int，`SteamAPICallCompleted_t.m_cubParam` 是 uint32）类型就不一致**，Valve 头文件内部就有 2GB/4GB 两个口径。

### A3. 分配失败行为

`Vec::resize` 分配失败走 `std::alloc::handle_alloc_error` → **默认 abort 进程**，不是可 `catch_unwind` 捕获的 panic。即：畸形值触发时进程直接终止，无法优雅降级。

另实测 Vec 缩容语义：

```
resize(64KB) 后: len=65536, cap=65536
再 resize(512) 后: len=512, cap=65536  ← cap 不缩
```

`apicall_result` 是循环外的局部 Vec（`lib.rs:163`），**一次畸形大值会让 4GB capacity 常驻**直到回调循环结束。

### A5. 上游时效性

- `gh issue list` 检索 `resize OR m_cubParam OR OOM OR allocation OR memory`：**无相关 Issue**。
- 当前源码（`lib.rs:170`）确认问题仍在，无上游修复尝试。

### C1. 重复检索

见上——6 条结果（#283/#302/#122/#232/#242/#159）全部无关（性能、构建、无关 unsound）。**无重复报告**。

---

## B. 可达性与严重度分析

### 真实触发条件

`m_cubParam` 由 Steam 客户端通过本机 IPC 返回。要出现畸形值需要：

1. Steam 客户端自身 bug（Valve 侧缺陷）
2. 本机 IPC 被篡改（攻击者已有本机权限，收益极低）
3. 正常回调就超过 2GB（**不存在**——已知最大回调为远程存储/UGC 文件传输的分块结果，量级在 MB 级）

**结论：现实可达性极低，维持「低」严重度正确。**

### 与 Valve 官方模板的对照（B1 类核查）

`steam_api.h:189`（Valve 官方模板）：

```c
void *pTmpCallResult = malloc( pCallback->m_cubParam );  // 同样无上限
```

**这改变了责任归属**：若在 Issue 中写「Rust 绑定缺少上限校验」，维护者可合理回应「Valve 官方模板也是这么写的」。准确的表述应该是：

- **可以提的点**：u32→i32 静默负数转换是 Rust 绑定特有的（C 模板里 `malloc` 和参数传递同为 int 无此口径分裂）；以及 `m_cubParam as usize` 与 `m_cubParam as _` 两侧口径不一致。
- **不应单独提的点**：「无上限校验」本身——那是 Valve 契约层面的既有假设。

---

## 修复建议（若提 PR）

最小修复——加一个合理性上限 + 统一转换口径：

```rust
// src/lib.rs:323
const MAX_CALL_RESULT_SIZE: usize = 64 * 1024 * 1024; // 64MB，覆盖所有已知回调
let size = apicall.m_cubParam as usize;
if size > MAX_CALL_RESULT_SIZE {
    // 记录日志，跳过该回调（与 Failed 路径一致）
    sys::SteamAPI_ManualDispatch_FreeLastCallback(pipe);
    continue;
}
apicall_result.resize(size, 0u8);
```

注意：
- 上限值需要论证（已知最大回调量级 + 余量），**不应拍脑袋定**。
- `64 * 1024 * 1024` 这类常数写进 PR 前应先枚举已知回调的最大 `m_cubParam`。
- 改动是纯内部逻辑，**零公开签名破坏**。

---

## 深挖产出的新事实（相比主文档第 0 节）

| # | 新事实 | 证据 | 影响 |
|---|--------|------|------|
| 1 | u32→i32 静默负数转换 | `cubparam_cast_test.rs` 实测 | 缺陷点比「无上限」更精确 |
| 2 | 两侧转换口径不一致（usize vs c_int） | lib.rs:170 vs lib.rs:176 | 真正的 Rust 侧特有问题 |
| 3 | Valve 官方模板同样无上限 | steam_api.h:189 | 责任归属修正，Issue 表述需调整 |
| 4 | 分配失败是 abort 不是 panic | Rust alloc 语义 | 无法 catch_unwind，畸形值=进程终止 |
| 5 | Vec 不缩容，畸形值后 capacity 常驻 | 实测 cap=65536 after resize(512) | 单次畸形的残留内存影响 |
| 6 | Valve 头文件自身 2GB/4GB 双口径 | CallbackMsg_t(c_int) vs SteamAPICallCompleted_t(uint32) | 转换风险源于 Valve 设计，非绑定引入 |

**严重度结论不变：低。** 触发需要 Steam 客户端返回畸形值，现实可达性极低。但修复成本也低（内部改动、零签名破坏），可与其它修复合并提交。

---

## 实施记录（2026-10-10）：上限 + 移入循环已在独立分支落地（`fix/call-result-size-cap`，commit `ca23aab`）

基于 master `6e4a2bc`，**不含**场景 E 修复（两条分支相互独立，各自一 PR）。改动 +35/-2，全部在 `src/lib.rs`：

- `MAX_CALL_RESULT_SIZE = 64 MiB` 模块级常量 + `call_result_size_ok(size)` 纯函数——除 64MiB 上限外同时要求 `size <= i32::MAX`：`cubCallback` 形参是 `c_int`，`u32→i32` 过 `i32::MAX` 即静默变负（上表新事实 #1/#2），一个守卫同时堵住两个转换缺陷点；
- `apicall_result` 移入循环内按次分配（新事实 #5 的 capacity 常驻随之消除）；超限走 `FreeLastCallback` + `continue`，每迭代仍恰好一次 free，与失败路径同形；
- 在树新增**无 Steam 单测** `call_result_size_bound`（顺带落实路线图 #9「无 Steam 环境可跑的单元测试」的第一格）。

**门禁同尺复测**（HEAD 基线取自同一 HEAD 的另一 worktree、共享 target）：build 通过 / test **8 通过**（基线 7 + 新增 1）/ 11 失败 / 2 忽略（失败项与基线同集，全部止步 `Client::init()`）/ clippy **121 与基线相同**、改动区（lib.rs 60-105/155-205/790-840）零命中 / fmt clean。

**验证边界**（与场景 E 修复同款口径）：`run_callbacks_raw` 需真 Steam pipe，本机无 Steam 客户端 ⇒ 证据强度 =「在树单测 + 无回归 + 结构检视」，**不是**真 Steam 环境端到端。
