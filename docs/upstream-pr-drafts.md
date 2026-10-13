# 上游 PR 草案（2026-10-11 定稿，待用户拍板推送）

> 上游仓库：`Noxime/steamworks-rs`（本地 origin）。全部分支基点 = master `6e4a2bc`（除非注明依赖分支）。
> 推送顺序建议按本文件排序（前七支互独立、master 基；第八支依赖 #3 支——即本文的第 3 支）。
> **A5 查重已完成（2026-10-11，网络恢复后实测）**：
> - 13 个开放 PR **无一与我们的缺陷范围重叠**；开放 issues 无同病报告（`panic`/`CString` 搜索仅 #115 使用咨询）。
> - **#340**（`must_use` for callback handles：callback.rs/lib.rs/server.rs/networking_sockets_callback.rs）与第 3 支**文件面重叠、语义正交**——若它先进，第 3 支 rebase 后补属性即可（trivial）；反之亦然。**#347**（input glyph segfault，仅 input.rs）与第 4 支同文件异机制，rebase 级。
> - **#253 仍 open、维护者未回复**（最后两条评论是我们的五案例取证）——第 1/8 支提交时显式关联 #253（第 8 支就是它的解法提案）。
> - **#332（SteamError 转换去 panic）已 merged** ⇒ 上游接受 panic 清理类 PR，第 5/6 支有先例可引。
> - Inventory 立项时：PR #205（closed 未合并、MERGEABLE、维护者已表态 API 偏好）仍是起点。
> 门禁数字均来自主文档「八分支集成栈验证」节与各分支提交 body，可在 PR 里引用。

---

## 1. `fix/call-result-handler-self-deadlock` @ `20b800a`（缺陷 #2 场景 E）

**标题**：run_callbacks: release the call-result lock before invoking user handlers

**Body 要点**：`SteamAPICallCompleted` 处理器在持有 `call_results` 锁的状态下被调用——
处理器内再调任何注册 CallResult 的 Steam API（例如回调里发起请求）即自锁死。
修法 = 缩小守卫作用域（取出 handler、放锁、调用），+10/−2 单点改动。
零公开 API 变更。附 A/B 复现（`repro/call_result_chain_deadlock_fixed.rs`）。
上游关联：#259（拆 Mutex 的 PR）引入了此锁序；本修复为其后续。

## 2. `fix/call-result-size-cap` @ `ca23aab`（缺陷 #3）

**标题**：Bound SteamAPICallCompleted::m_cubParam before resizing the result buffer

**Body 要点**：`m_cubParam` 是 Steam 直给的 `uint32`——损坏值（实测可达 `usize::MAX`）
驱动 `apicall_result.resize` 巨额分配，分配失败直接 abort（不可捕获）。
修法：`MAX_CALL_RESULT_SIZE = 64MiB` 上限 + `call_result_size_ok` 兼管 `i32` 负值口径
（u32→i32 静默变负）+ 缓冲移入循环避免每次重分配。含在树离线测试 `call_result_size_bound`。

## 3. `fix/callback-multi-registration` @ `5d2bcc9`（缺陷 #1）

**标题**：Support multiple callbacks per callback type; stop handle drop from killing others

**Body 要点**：同类型第二次注册会静默覆盖第一次（文档化的替换语义仅覆盖 3 个 API，
其余全部误伤）。表改 `HashMap<i32, Vec<(u64, Box<dyn FnMut>)>>` + 每注册一个 seq，
`CallbackHandle` 按 seq 精确注销（陈旧句柄零误杀）；新增
`register_replacing_callback` 承接三处文档化替换语义（顺带消除 5 处 mem::forget 句柄泄漏）。
行为变更：同类型多回调共存 + 按注册顺序派发。6 文件 +277/−42。

## 4. `fix/null-guard-unavailable` @ `6abc953`（缺陷 #4；两提交）

**标题**：Make interface getters degrade instead of asserting on null pointers

**Body 要点**：Steam 不暴露某接口访问器时 getter 返回 null，后续 `assert!(!ptr.is_null())`
在 **release** 下编译掉 ⇒ 空指针解引用 SIGILL 杀进程（实测复现）。
修法：23 个接口结构体加 `unavailable` 标记（timeline 式先例：该模块已用 disabled 语义），
258 个方法入口守卫 → no-op 降级（bool→false / Option→None / 句柄→0）。
零 panic、零签名变更。20 文件 +1041/−0。

## 5. `fix/cstring-nul-panic` @ `5bf2336`（缺陷 #5）

**标题**：Degrade instead of panicking when CString conversion fails

**Body 要点**：58 处 `CString::new(...).unwrap()`（入参含 NUL 即 panic，编译期不可见）
改为 `pub(crate) cstring()` helper + 按返回通道降级（bool→false / Option→None /
Result→既有 Err / 句柄→0 / 结构体名→空 / 回调→cb(Err(IOFailure))）。
19 处 UGC builder 的 panic 点不在本 PR（见第 6 支的破坏性版本）。
16 文件 +424/−70。（注：上游若偏好整套走 #6 的 Result 化，本支可让出 UGC 部分——按 A5 结果定。）

## 6. `fix/ugc-builder-result` @ `f54fb6c`（缺陷 #6）⭐破坏性

**标题**：Return Result from UGC builder methods instead of panicking

**Body 要点**：22 个 builder 方法（13 UpdateHandle + 9 QueryHandle）
`-> Self` 改 `-> Result<Self, SteamError>`：NUL→`InvalidParameter`、
canonicalize NotFound→`FileNotFound` 其余 io→`IOFailure`、sys `Set*` 假→`InvalidParameter`。
附带行为改善：QueryHandle 串方法 release 下 `debug_assert!` 编译掉的静默失败改回 Err；
`SteamParamStringArray::new` → `Option`（失败回收已转换串）；
`init_for_game_server` 保 `-> bool`（NUL→false）避免级联 Server 签名。
**破坏性变更，搭 major 版本窗口**（维护者 #253 的 option 4 正在规划破坏性变更）。
离线测试 8 个（null ISteamUGC + 错误路径先于 sys 调用）。

## 7. `fix/test-tiering` @ `7cd6452`（缺陷 #9）

**标题**：Tier the test suite for Steamless CI and add a test gate

**Body 要点**：现 CI 只有 fmt/build——`cargo test` 从不跑（55% 测试无 Steam 会挂）。
修法：11 个 Steam 依赖测试 `#[ignore = "requires a running Steam client"]`（`--ignored` 可跑）；
3 个调 `Client::init()` 的文档示例改 `no_run`（编译检查保留）；
`ci.yml` 新增 test job：`cargo test`（pipefail+tee）+ 断言每个 result 段 ok 且 >0 passed 0 failed、
ignored 数回显日志（只跳不跑不算绿）。附带发现记入 body：cargo test 无 `--no-fail-fast`
首段失败即停 ⇒ Doc-tests 段此前从未被执行。

## 8. `fix/callback-dispatch-reentrancy` @ `ebd1183`（缺陷 #2 残余 B/C/D）⭐**依赖第 3 支**

**标题**：Fix deadlock on re-entrant calls from inside a callback

**Body 要点**：dispatch 持表锁调用户代码 ⇒ 回调内注册 / 嵌套派发 / 丢自己句柄三者死锁。
修法：put-back（整桶取出→放锁调用→按 seq 归并回插）+ `pending_removal` 墓碑
（回调内丢句柄/替换注册在在途期依然生效）。零公开 API 变更。
语义变更记档：回调 panic 丢弃在途条目而非毒化锁。
**基于 `fix/callback-multi-registration` 尖端（5d2bcc9）**——上游 PR 须在其合并后提交
（或 GitHub 上设 base 分支为该 PR）。

---

## 已推送并开 PR（2026-10-11，fork `bfxh/steamworks-rs`）

| 分支 | PR |
|------|----|
| call-result-handler-self-deadlock | [#350](https://github.com/Noxime/steamworks-rs/pull/350) |
| call-result-size-cap | [#351](https://github.com/Noxime/steamworks-rs/pull/351) |
| callback-multi-registration | [#352](https://github.com/Noxime/steamworks-rs/pull/352) |
| null-guard-unavailable | [#353](https://github.com/Noxime/steamworks-rs/pull/353) |
| cstring-nul-panic | [#354](https://github.com/Noxime/steamworks-rs/pull/354) |
| test-tiering | [#355](https://github.com/Noxime/steamworks-rs/pull/355) |
| ugc-builder-result | [#356](https://github.com/Noxime/steamworks-rs/pull/356) |
| callback-dispatch-reentrancy | [#357](https://github.com/Noxime/steamworks-rs/pull/357)（叠在 #352 上，先合 #352；squash 合并则 rebase force-push） |
| offline-accessor-coverage | [#358](https://github.com/Noxime/steamworks-rs/pull/358) |

PR body 全文存 `pr-bodies/`。执行备注：#356 首建撞 504（`| tail` 吃掉退出码，#355 先出）——**教训：gh 管道别接 `| tail`，退出码会被吞**；#357 body 的依赖说明已按实际 base=master 修正。

## 拍板清单

| # | 分支 | 基 | 破坏性 | 建议 |
|---|------|----|--------|------|
| 1 | call-result-handler-self-deadlock | 6e4a2bc | 否 | PR |
| 2 | call-result-size-cap | 6e4a2bc | 否 | PR |
| 3 | callback-multi-registration | 6e4a2bc | 内部表+新 API（CallbackHandle 语义收紧） | PR |
| 4 | null-guard-unavailable | 6e4a2bc | 否 | PR |
| 5 | cstring-nul-panic | 6e4a2bc | 否 | PR（或并入 6 讨论口径） |
| 6 | ugc-builder-result | 6e4a2bc | **是** | PR，指明搭 major 窗口 |
| 7 | test-tiering | 6e4a2bc | 否 | PR |
| 8 | callback-dispatch-reentrancy | **5d2bcc9** | 否 | PR，排在 3 之后，关联 #253 |
| 9 | offline-accessor-coverage | 6e4a2bc | 否（tests-only） | PR，可任意顺序 |

**第 9 支草案**：`test/offline-accessor-coverage` @ `060ad3f`——"Add offline tests for the pure-logic accessor surface"。12 个离线测试锁定 `AppNetConnectionEnd` 双区间 panic 契约（4 个 should_panic 边界）、`LobbyKey` 255 字节边界、`StringFilterKind→ELobbyComparison` 六臂全映射、`data_type` 三组抽查、`AppIDs` 三变体、`LobbyListFilter` 构建链。3 文件 +164/−0；串行 19-11-2 = 7-11-2+12；clippy 多重集逐项同；并行 10×0。

集成验证：`integration/stack8` @ `c8b80c9`（**九支全并**、串行 **52 通过/0 失败/13 忽略** + Doc 14/0、exit 0；clippy 多重集对 base 0 新增 1 消失；第 9 支并入仅 ugc.rs 尾部 keep-both 冲突 + 修一处插入缺损）。
