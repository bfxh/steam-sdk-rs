# SuperSteam: 下一代 Steamworks Rust 绑定 — 设计与实证分析

> 本文所有关于 steamworks-rs 的论断均标注了实测证据（文件:行号）。
> 关于 Steamworks.NET 的论断来自公开仓库源码与 Issue 实证。
>
> **行号对照基准**：`steamworks-rs` **工作区**（含本地未提交修改：`src/lib.rs` +4 行的 callback_stream 接线、新增 `callback_stream.rs`/`stream_events.rs`）。
> 若对照上游 HEAD（`6e4a2bc`），`lib.rs` 行号需 **−4**；`callback.rs`/`server.rs`/各头文件无本地修改，行号两边一致。
> 复现脚本与 grep 命令以工作区为准。
>
> **坐标更新（2026-10-15 按内容重测，master 已到 `467ab19`）**：本文行号是**当时**的实测坐标；此后源码做了目录化拆分与多处修复，以下落点是按内容重新定位的（不是按文件名猜的）。
> - `src/friends.rs` 已不存在，分为 `src/friends/mod.rs`（2026-10-16 复测：它现在只剩 70 行测线——根里那 48 个 `pub fn` 已按关注点拆进 `list.rs` 85、`overlay.rs` 88、`rich_presence.rs` 77、`clans.rs` 174、`clan_chat.rs` 157、`followers.rs` 91、`messages.rs` 60、`flags.rs` 50 八件，`impl Friends` 变成 8 个块（`grep -rc '^impl Friends' src/friends/*.rs` 实测 8 个文件各 1 块））、`friend.rs`（`Friend` 句柄，401 行）、`callbacks.rs`（10 个回调载荷，155 行）、`chat.rs`（216 行）。原 `friends.rs:64-68` 的 `CStr::from_ptr` + `to_string_lossy` 读法现在在 `friend.rs:38`、`:50`、`:198` 与 `callbacks.rs:67`；原 `friends.rs:412/433/454` 的 `vec![0; 32*32*4]` 头像缓冲现在只剩一处，`friend.rs:111-112`；原 `friends.rs:124` 那类 `CString::new(..).unwrap()` 站点现在在 `friend.rs:172`、`:191`（`:277` 那处在测试里）。
> - `src/ugc.rs` 已不存在，分为 `ugc/mod.rs`、`update.rs`、`query.rs`、`results.rs`、`answers.rs`、`types.rs`、`read.rs`、`ugc_tests.rs`。原 `ugc.rs:974` / `:986-997` 那两个 `canonicalize().unwrap()` 站点**已不再是站点**：路径解析收敛到 `src/ugc/update.rs:26` 的 `resolved_path`（NotFound→`FileNotFound`，其它 IO→`IOFailure`），两个 setter 各自 `?` 它。原 `ugc.rs:933-1050`「15+ 个构建方法返回 `Self` 而非 `Result`」**今天实测**：`update.rs` 21 个方法返回 `Result<Self, SteamError>`、**0 个返回 `Self`**，且文件里已无任何真实 panic 站点（`assert!`/`unwrap`/`expect` 三处命中全在文档注释里）；`query.rs` 现 **30 个方法返回 `Result<Self, SteamError>`、0 个返回 `Self`**，且文件内已无 `unwrap`/`expect`/`assert!`（唯一命中是我写的那句说明文字）；`fetch`/`fetch_total`/`fetch_ids` 保持回调签名，失败经 `cb(Err(..))` 送达。转换次序：8 个早于本轮、`visibility`/`add_content_descriptor`/`remove_content_descriptor` 三个在 `467ab19`、其余 10 个在本次改动。
> - `src/lib.rs` 已瘦身三次（身份类型→`src/identity.rs`，根测试→`src/lib_tests.rs`）。原 `lib.rs:780` 的 `basic_test` 现在 `src/lib_tests.rs:12`；原 `lib.rs:322-323` 的 call_results「命中即摘除」现在 `src/lib.rs:326-327`；原 `lib.rs:508-518` 的 timeline `disabled` 标记现在在 `src/timeline.rs:7`（`lib.rs` 里已无 timeline 实现）；接口 getter 现在 `lib.rs:498`（utils）与 `:558`（friends），`debug_assert!(!x.is_null())` 守卫仍在原地（如 `lib.rs:501/513/534/549`）。
> - **最大的单文件模块已目录化（不是全部）**（2026-10-16 用 `git ls-files` + `wc -l` 重测，不是按文件名猜；
>   `ls src/*.rs | wc -l` 实测仍有 **24 个**单文件模块直接躺在 `src/` 下，最大的是 `input.rs` 725、
>   `error.rs` 603 等叶子模块——`input.rs` 725 与 `utils.rs` 710 已在 2026-10-16 分别拆进 `src/input/`、`src/utils/`，所以现在的第二名是 `networking_types/config.rs` 629）：除上面的 `friends`、`ugc` 外，`src/user/`（`mod.rs` 375、`voice.rs` 437、`duration.rs` 393、`auth.rs` 244、`buffers.rs` 206、`passthrough.rs` 69、`answers.rs` 62、`callbacks.rs` 61）、`src/user_stats/`（`leaderboard.rs` 384、`stats.rs` 276、`global_stats.rs` 280、`stat_callback.rs` 151、`stats_io.rs` 127、`achievements.rs` 90、`mod.rs` 20）、`src/matchmaking/`（478/314/280/205/111）、`src/inventory/`（338/204/132/121/101/83/44）、`src/networking_types/`（`mod.rs` 36，其余见目录）、`src/networking_sockets/`（`connection.rs` 536、`mod.rs` 260、`fake_udp.rs` 317、`fake_ip.rs` 195、`listen_socket.rs` 122、`listen.rs` 120、`poll_group.rs` 102、`status.rs` 98、`connect.rs` 92、`auth.rs` 49）、`src/utils/`（`text_input.rs` 178、`mod.rs` 142、`filtering.rs` 103、`ipv6.rs` 60、`identity.rs` 56、`warning.rs` 54、`overlay.rs` 47、`param_strings.rs` 47、`client_state.rs` 42、`battery.rs` 34、`callbacks.rs` 29）、`src/input/`（`origins.rs` 178、`actions.rs` 104、`layers.rs` 96、`effects.rs` 87、`callbacks.rs` 83、`roster.rs` 67、`core.rs` 58、`session.rs` 57、`mod.rs` 53）、`src/html_surface/`（`browser.rs` 352、`callbacks.rs` 247、`mod.rs` 206、`keys.rs` 63）、`src/remote_storage/`（`mod.rs` 247、`files.rs` 247、`api.rs` 182、`visibility.rs` 68、`changes.rs` 58、`platforms.rs` 27）、`src/server/`（`config.rs` 172、`lifecycle.rs` 170、`callbacks.rs` 128、`interfaces.rs` 130、`auth.rs` 107、`mod.rs` 105、`packets.rs` 59；`interfaces.rs` 里那段 `/* TODO: Buggy currently?` 就是原文里被注释掉的 `Server::apps()`，它一直是注释不是代码）。**测试名跟着模块路径走**：下文当日测试日志里的 `user_stats::test_global_stat_cstring_error` 今天叫 `user_stats::global_stats::test_global_stat_cstring_error`，`user_stats::test`（`#[ignore]`）今天叫 `user_stats::leaderboard::test`；日志原文不改写。
> - **已经不再成立的论断**：手写源码里的 `unreachable!()` 位点现在是 **0**（`scripts/ci/baselines/no_panic_sites.txt` 为空，这条门的目标值本来就是 0），文中「10 处 / 9 处 / 2 处」都是当时计数；`Friend::state()`、`EPersonaState::k_EPersonaStateMax`、`k_EWorkshopFileTypeMax`、`ERemoteStoragePublishedFileVisibility`、`EUGCContentDescriptorID` 这几条兜底臂都改成了具名 `Unknown`。测试口径也变了：lib 套件现在实测到达 155 个（140 passed + 15 ignored，`bash scripts/ci/test_floor.sh lib` 于 2026-10-16 复测；同一晚先记 129、再记 153，两次都被后续提交超过，差额没有逐项归因，所以只换数不换解释），原文「20 个测试中 `basic_test` 依赖真实客户端、14 个模块零覆盖」是 `#[ignore]` 机制引入前的状态。
> - **本轮未重测、因此别当现状读**：缺陷 #5 的 `CString::new(..).unwrap()` 站点总数（原文 77 处；本轮单行 grep 得 79，但跨行链式的口径没有复算，所以不拿它替换原数）、缺陷 #8 的 `Arc<Inner>` 克隆计数，以及 `defect-0N-*.md`、`ci-local-evidence-2026-10-13.md` 里的全部行号——那些文件是**当日证据记录**，保留原坐标不改动，引用时按上面的对照表落地。
>
> **复现脚本位于 `repro/` 目录**（独立可编译，无需 Steam 客户端）：
> - `repro/deadlock_test.rs` —— 缺陷 #2 死锁复现（镜像**修复前**的单 Mutex 模式，即 PR #259 之前）
> - `repro/post_fix_deadlock_test.rs` —— 缺陷 #2 **修复后**的剩余影响面实测（A 已修 / B,C,D 仍死锁）
> - `repro/cstring_panic_test.rs` —— 缺陷 #5 CString panic 复现（镜像 `src/friends.rs:124`）
> - `repro/resize_oom_test.rs` —— 缺陷 #3 resize 行为实测（4 种尺寸 + 类型语义）
> - `repro/cubparam_cast_test.rs` —— 缺陷 #3 补充：u32→i32 转换负数实测（cubCallback 口径不一致）
> - `repro/arc_clone_bench.rs` —— 缺陷 #8 性能实测（1 亿次迭代）
> - `repro/arc_validity_check.rs` —— 缺陷 #8 测量有效性检验（排除编译器优化干扰）
> - `repro/null_guard_test.rs` —— 缺陷 #4 release 空指针实测（需分别以 debug/release 编译模式运行）
> - `repro/ugc_builder_panic_test.rs` —— 缺陷 #6 builder 链 panic 面实测（归因订正：Set* 为暂存语义，非不可回滚，见 defect-06 档）
> - `repro/callback_overwrite_test.rs` —— 缺陷 #1 静默覆盖 + 陈旧 Handle 误杀复现
> - `repro/call_result_chain_deadlock_test.rs` —— 缺陷 #2 场景 E：call_result 内链式调 API 同锁自锁（含跨锁对照组）
> - `repro/call_result_chain_deadlock_fixed.rs` —— 缺陷 #2 场景 E **修复 A/B**（E-before 死锁 / E-after 正常），对应 `src/lib.rs` 已实施的守卫作用域收缩
> - `repro/stream_deadlock_demo.rs` —— CallbackStream（本地 futures 封装）的挂起语义实测：空队列 `block_on(pending)` 死锁 vs `now_or_never()` 返回 None

---

## 0. 实证结论摘要

| # | 发现 | 证据 | 严重度 |
|---|------|------|--------|
| 1 | 同一回调类型重复注册会静默覆盖前一个；**陈旧 Handle drop 会按 ID 误杀新注册；内部自动注册与用户注册同表互杀** | src/callback.rs:198 `HashMap::insert`、callback.rs:167 `remove`（已复现，见 defect-01 文档） | 高 |
| 2 | 可重入死锁：持锁运行用户闭包。**PR #259 已修复「回调内调 API」场景；仍存「回调内注册回调/嵌套派发/丢弃 Handle」+ 新发现「call_result 内链式调 API」共四种死锁**（**2026-10-14 更新：这四种已全部判死，见下方"缺陷 #2 状态"**） | Issue #253 + PR #259（已合并）；实测见 0.6 节与 defect-02 文档 | 高（**E 场景使影响面回升**） |
| 3 | `apicall_result.resize(m_cubParam)` 无上限校验 + u32→i32 静默负数转换 | src/lib.rs:170/176（已实测，见 0.4 节） | 低 |
| 4 | 接口指针仅 `debug_assert` 判空，release 下裸指针解引用 | `null_guard_test.rs` 实测：release 下 **Illegal instruction (退出码 132)** | 高（已复现） |
| 5 | 77 处 `CString::new(...).unwrap()` 遇内嵌 NUL 直接 panic（原 73，漏 4 处跨行链式） | src/friends.rs:124 等 15 文件（已复现，双口径复核见 defect-05） | 高 |
| 6 | UGC builder 方法失败即 panic（`canonicalize().unwrap()` + `assert!`，**25 个 panic 点**） | `ugc_builder_panic_test.rs` 实测；**归因订正：暂存未提交，公开状态未变更**（见 defect-06 文档） | 中（已复现） |
| 7 | 接口覆盖 **23/34**（宽口径；原 29 分母漏 5 个 `_SteamAPI_` 中缀接口）；缺口 **11**（口径不变量）；必补仍为 **Inventory、HTTP、ParentalSettings**，**Inventory 有停滞已测实现 PR #205** | 已实测（见 6.1 节与 defect-07 文档） | 中（补齐成本低于预估） |
| 8 | 每个接口方法调用都克隆 `Arc<Inner>`（42 处 `self.inner.clone()`，口径见 0.5 节） | src/lib.rs:355 等（已实测，见 0.5 节） | **性能论据不成立** |
| 9 | 20 个测试 / 14197 行；实测 `cargo test` **11 失败 7 通过 2 忽略**，失败全部因无 Steam 客户端 | 已实测（见 0.3 节） | 中 |
| 10 | Steamworks.NET 每次字符串调用 `AllocHGlobal`，`SetStat` 慢问题 2016 年至今未修 | Steamworks.NET Issue #106 | 高（对比项） |

### 缺陷 #2 状态（2026-10-14，四种自重入场景已闭合）

改法是"同上下文延后 + 嵌套派发拒绝进入"，而不是去持锁运行用户闭包这件事本身：

- **回调内注册回调 / 回调内丢弃 Handle** —— 派发循环持 `callbacks` 锁运行用户闭包，
  同一线程再取同一把锁必自锁。现在注册与移除都走 `Callbacks::run_or_defer`：
  如果本线程正为该上下文派发，操作进 FIFO 队列，由派发循环在每条回调
  `FreeLastCallback` 之后 `drain()` 落地（所以"回调里注册的回调"在同一轮派发里就能生效，
  也不会丢注册）。不同上下文（客户端与专用游戏服务器各一套锁）不延后，立即执行——
  否则会等一场可能永远不来的派发。
- **嵌套派发** —— `DispatchGuard::try_new` 对同一上下文第二次进入直接返回 `None`，
  于是回调里再调 `run_callbacks()` 变成 no-op：外层循环本来就在抽同一条队列，
  事件不会漏，而重入只会死锁。
- **call_result 内链式调 API（场景 E）** —— 早先的"守卫作用域收缩"已修（把 handler
  在锁内取出、锁外调用），本轮未改动该路径。

**证据边界要写清**：以上由 3 个离线单测钉住（`deferral_is_per_context_and_drains_in_arrival_order`、
`deferred_registration_lands_in_the_handler_map_and_fires`、`nested_dispatch_of_same_context_is_refused`），
并且逐个做过变异验证——把 `run_or_defer` 改成永不延后、把 `try_new` 改成永不拒绝、
把 `clamp_viewers` 的判据改掉，对应测试各自判红。**没有**做的是端到端实测：
真正的派发循环要 `SteamAPI_ManualDispatch_*` 与真实 pipe，本机无 Steam 客户端跑不到，
所以"接了真客户端就不再挂死"这一条目前只有机制级证据。

**仍然存在的已知约束**（不是死锁，是阻塞）：别的线程在本线程派发期间注册回调，
会等 `callbacks` 锁直到当前这条回调执行完——上界是单条回调的耗时。彻底消除要把
派发改成无锁快照（见第 5 节目标形态）。

### 一处自我更正

在初步分析中，我曾断言 `SteamAPI_ManualDispatch_Init` 不存在于 Rust 绑定（基于在预编译 `libsteam_api64.dll` 导出表里找不到该符号）。

**这个判断是错的。** 头文件 `steam_api.h:210` 明确定义了该函数，`windows_bindings.rs:19611` 也正确生成了绑定，`src/lib.rs:264` 确实调用了它。导出表搜不到的原因是该符号可能通过序号或宏转发导出。事实是：**Rust 侧调用是正确的，C 侧实现存在**。

我把这条更正明确写在这里，因为「凭已编译二进制导出表下结论」正是容易产生假结论的取证方式——我在这个项目里踩了一次。

### 第二处自我更正：回调「数据竞争」的说法不成立

我最初把缺陷 #2 描述为「回调变量数据竞争」：声称 `src/lib.rs:162` 的单个 `callback` 变量在持锁运行用户闭包期间会被 API 覆写，多线程调用必竞态。

**复现后这个说法是错的**，理由有三：

1. `callback` 是 `run_callbacks_raw` 的**栈上局部变量**（lib.rs:162），每个调用线程有独立栈帧，不存在跨线程共享，因此不存在数据竞争。
2. `apicall_result`（lib.rs:163）同样是局部变量，同理。
3. 真正的竞争面在 Steam API 内部缓冲区的归属契约上，而 Valve 的头文件**没有**声明 `ManualDispatch` 的线程安全性，因此无法从 Rust 侧代码断定「必竞态」——那属于推测，不是实证。

我为此写了复现程序（`deadlock_test.rs`，忠实镜像 lib.rs:104-131 的锁模式）去实测，结果暴露出一个**真实且可复现**的缺陷：

```
  用户回调已进入
  回调内部尝试注册新回调...
  结果：死锁 —— 3 秒未返回，线程永久阻塞
  成因：std::sync::Mutex 不可重入，回调内注册/派发必死锁
```

**真实缺陷是可重入死锁，不是数据竞争。** 机制如下：

- `src/lib.rs:255-257` 的 `lock()` 作用域**覆盖了整个 `cb(data)` 用户闭包调用**（`process_callbacks` 版本在 lib.rs:146-149 同样如此）。
- 用户在任何 Steam 回调内部调用 `register_callback`（或 `run_callbacks`）都需要同一把 `callbacks` 锁。
- `std::sync::Mutex` 不可重入 → 自锁 → 线程永久挂起。

值得注意的是，Valve 官方的手动派发模板（`steam_api.h:173-203`）里**没有**这个死锁风险，因为纯 C 写法不需要持锁。这个缺陷是 Rust 绑定为满足 `FnMut` 的可变性约束而引入的——属于实现层问题，而非 API 设计问题。

我保留这条更正，因为它示范了「读代码得出竞态结论」与「跑出可复现证据」之间的差距：前者给出的是听起来专业的推测，后者才给出能验证的事实。文档第 0 节的缺陷表已按实测结果改写。

---

## 0.1 缺陷 #5 实测证据（CString panic）

复现程序 `cstring_panic_test.rs` 忠实镜像 `src/friends.rs:124` 的模式，实测输出：

```
输入: "normal_text\0evil_payload"

thread 'main' panicked at cstring_panic_test.rs:8:39:
called `Result::unwrap()` on an `Err` value: NulError(11, [110, 111, ..., 0, ...])
```

**这不是理论风险，而是可实测的 panic。** 关键证据是调用点分类：

| 分类 | 数量 | 参数来源 | 可达性 |
|------|------|---------|--------|
| 公开 API 的 `&str` 入参 | 绝大多数 | 完全由调用方（游戏代码）控制 | **真实可达** |
| 内部常量（如 `networking_sockets.rs:1076` 的 `"Handle was dropped"`） | 极少数 | 编译期字面量，不含 NUL | 不可达 |

因此「77 处」这个数字需要精确表述：**83 处 `CString::new` 调用中，77 处带 `unwrap()`/`expect()` 是真 panic 点；另 6 处非 panic**（`user_stats.rs:386/418/456/496` 用 `.map_err(|_| ())?`，`ugc.rs:1100` 用 `.ok()`，`user_stats.rs:867` 是测试断言）。**双口径复核**：窄正则（单行 grep）得 73，跨行扫描得 77，差的 4 处是 `.expect()` 跨行链式调用（`ugc.rs:1209/1222/1243`、`utils.rs:320`），见 defect-05 复核文档。

**计数口径**：77 = 带 unwrap/expect 的 panic 点；83 = 全部 CString 调用；6 = 非 panic（含 1 处测试断言）。跨 15 文件。窄正则会漏 4 处跨行 `expect`，故单行 grep 得 73（ugc.rs 17 + utils.rs 2），跨行扫描得 77（ugc.rs 20 + utils.rs 3），见 defect-05。

修复面分布（按文件）：

| 文件 | panic 点数 | 备注 |
|------|-----------|------|
| `ugc.rs` | 21 | 含 builder 链（见下节） |
| `user_stats.rs` | 12 | 部分已改 Result，其余仍 panic |
| `server.rs` | 11 | GameServer 侧 |
| `matchmaking.rs` | 11 | 大厅数据读写 |
| `friends.rs` | 8 | overlay/rich presence |
| `timeline.rs` / `input.rs` | 4 / 4 | |
| `utils.rs` / `networking_sockets.rs` | 3 / 3 | |
| 其余 6 文件 | 各 1 | |

### 真实触发场景（非理论）

1. **玩家名称含 NUL** → 好友系统、大厅成员数据（`set_lobby_member_data`）
2. **UGC 标签/metadata** → Workshop 物品上传（`add_key_value_tag`、`metadata`）
3. **网络反序列化数据** → P2P/大厅消息解析后含 NUL
4. **服务器 token** → `log_on(token)` 读配置文件（server.rs:419）

1. **玩家名称含 NUL** → 好友系统、大厅成员数据（`set_lobby_member_data`）
2. **UGC 标签/metadata** → Workshop 物品上传（`add_key_value_tag`、`metadata`）
3. **网络反序列化数据** → P2P/大厅消息解析后含 NUL
4. **服务器 token** → `log_on(token)` 配置文件读取（server.rs:419）

### 恶化因素：UGC builder 的链式 panic

`src/ugc.rs:933-1050` 的 15+ 个构建方法（`title`/`description`/`metadata`/`content_path` 等）返回 `Self` 而非 `Result<Self, E>`：

```rust
// src/ugc.rs:933
pub fn title(self, title: &str) -> Self {          // 无 Result
    let title = CString::new(title).unwrap();      // 中途 panic
    ...
}
```

后果是**构建链无法优雅降级**：用户已链式调用 5 步，第 6 步因输入含 NUL 直接 panic，前 5 步的暂存被放弃（**非「已提交无法回滚」**——`Set*` 暂存、`SubmitItemUpdate` 才 commit，未提交则公开状态未变更，见 0.1.x 归因订正），且调用方没有任何编译期提示知道该方法可能 panic。

相比之下，`activate_game_overlay`（返回 `()`）和 `set_lobby_data`（返回 `bool`）虽然也会 panic，但至少函数边界清晰。Builder 链式 panic 是这组缺陷中体验最差的形态——这也是 SuperSteam 将 UGC builder 全面改为 `Result` 返回的直接依据。

---

## 1. 设计理念与目标

SuperSteam 是一个全新的 Steamworks SDK Rust 绑定库，目标是在**正确性、性能、安全性与覆盖度**四个维度上同时超越 steamworks-rs 和 Steamworks.NET。

### 设计定位：仓内先例盘点结果

在动手设计前先做了一次「仓内已有优良做法」的盘点，避免重复发明。结论分三类：

| 类别 | 具体项 | 处置 |
|------|--------|------|
| **可直接沿用** | 零拷贝读法（`CStr::from_ptr` + `to_string_lossy`，14+ 处）、Timeline 的 `disabled` 运行时判空、`steamid_test` 式纯逻辑测试 | 直接借鉴，不重新设计 |
| **需补强** | 写入侧 `CString`（77 处 panic）、builder 链（39 处返回 `Self`）、接口判空（23 处 `debug_assert!`） | 这是 SuperSteam 的真实价值点 |
| **仓内无先例** | 同类型多监听者（`Vec<Arc<dyn Fn>>`，现为 `HashMap` 单值）、异步适配层、GameServer 对称支持、Inventory/HTTP/ParentalSettings | 需新设计 |

这个盘点的直接产出是修正了两处我的错误判断：

1. **#4 不必改公开签名** —— Timeline 已有零破坏的运行时判空先例（见 0.7 节）
2. **字符串「每次分配」的描述不成立** —— 读侧已是零拷贝，只有写侧有问题（见 4.2 节）

**共同教训**：这两处错误的模式相同——**只盯着要修的地方，没先盘点仓内已有实现**。写「改进方案」前应先回答「仓内是否已有同类问题的正确处理方式」，这既能降低工作量，也能避免把已有优点当成缺陷来描述。

### 核心目标

1. **正确性优先**：消除上述 10 项实证缺陷，尤其是回调静默覆盖与可重入死锁
2. **性能**：零成本抽象。注：原定目标「消除每次调用的 Arc 原子操作」经实测占帧预算仅 0.0005%，**性能论据不成立**（见 0.5 节）。改用 `Box<Inner>` 的真实理由是类型安全与 API 清晰度，不是性能
3. **安全**：公开 API 零 panic，unsafe 隔离在单一 FFI 模块
4. **覆盖**：全部 29 个 Steam 接口 + GameServer 全套
5. **易用**：原生 async/await + 构建器 + 交互式文档

### 三者对比（实证版）

| 维度 | steamworks-rs | Steamworks.NET | SuperSteam |
|------|---------------|----------------|------------|
| 回调正确性 | 可重入死锁 + 静默覆盖 | 委托注册（同类多次注册行为未定义） | 无锁派发 + 多监听者，可重入安全 |
| 接口覆盖 | 23/34 | 32 个生成类（近全量） | 近全量 + GameServer |
| 字符串处理 | 77 处 panic 点 | 每次 AllocHGlobal（慢） | 内联 C 缓冲，零 panic 零分配 |
| 错误处理 | 强类型枚举（优） | bool/EResult 原样（弱） | 分层错误 + 强制 Result |
| 内存安全 | unsafe 暴露（415 处 unsafe 块）；42 处 Arc 克隆（开销 8ns/次，实测不构成瓶颈） | finalizer 手动释放（易泄漏） | RAII，公开 API 零 unsafe |
| 异步 | 回调闭包 | 无 Task 支持 | 原生 async/await |
| 测试 | 20 个，14 模块零覆盖 | 仓库零 CI workflow | CI 门禁 + 100% 模块覆盖 |
| 维护 | 多人贡献较均衡 | 单人 90% 提交（bus factor=1） | 文档化架构降低门槛 |

---

## 2. 架构设计

### 2.1 四层架构

```
┌──────────────────────────────────────────────┐
│               Application                     │
├──────────────────────────────────────────────┤
│            SuperSteam API (用户层)            │
│  async/await │ 构建器 │ 类型安全 │ 流式事件    │
├──────────────────────────────────────────────┤
│           SuperSteam Core (核心层)             │
│  接口注册表 │ 回调队列 │ 事件总线 │ 生命周期    │
├──────────────────────────────────────────────┤
│        SuperSteam FFI (安全 FFI 层)            │
│  代码生成绑定 │ C 字符串缓冲 │ 错误映射 │ 守卫   │
├──────────────────────────────────────────────┤
│            Steamworks SDK (原生 C API)         │
└──────────────────────────────────────────────┘
```

### 2.2 关键设计决策

#### 决策 1：回调分发改为「无锁派发 + 可重入安全」

针对实证缺陷 #1（静默覆盖）与 #2（可重入死锁）。

```rust
pub struct Dispatcher {
    /// 同类型多个回调共存，不静默覆盖（修 #1）
    handlers: RwLock<HashMap<i32, Vec<Arc<CallbackFn>>>>,
}

impl Dispatcher {
    pub fn run(&self) {
        loop {
            // 阶段 1：仅持锁克隆要派发的处理器，随即释放锁
            let pending: Vec<(CallbackId, Arc<CallbackFn>)> = {
                let raw = unsafe { ffi::next_callback()? };
                let guard = self.handlers.read().unwrap();
                guard.get(&raw.discriminator)
                     .map(|v| v.iter().map(|h| (raw.id, h.clone())).collect())
                     .unwrap_or_default()
                // 锁在此处释放
            };

            // 阶段 2：锁外执行用户闭包 → 回调内注册/派发不再自锁（修 #2）
            for (id, handler) in pending {
                handler.invoke(id);
            }

            unsafe { ffi::free_last_callback() };
        }
    }
}
```

关键点：
- 用 `Vec` 值替代单值 `HashMap::insert`，从根上消除静默覆盖。
- 用 `Arc<CallbackFn>` 使处理器可在锁外克隆，闭包执行期间不持锁，从根上消除重入死锁。
- 文档须明确：锁在用户代码执行前已释放，因此回调内部可安全调用 `register_callback`。

#### 决策 2：所有公开 API 返回 `Result`，零 panic

针对实证缺陷 #5 和 #6。

```rust
/// 替代 77 处 CString::new().unwrap()
#[inline]
pub(crate) fn c_str(s: &str) -> Result<CString, SteamError> {
    CString::new(s).map_err(|_| SteamError::InvalidParameter)
}

// UGC builder 改为可失败（修 #6）
impl UpdateBuilder {
    pub fn content_path(mut self, path: &Path) -> Result<Self, SteamError> {
        let canonical = fs::canonicalize(path)
            .map_err(|e| SteamError::IOFailure(e.to_string()))?;
        let c = c_str(&canonical.to_string_lossy())?;
        unsafe { assert_ok(sys::SteamAPI_ISteamUGC_SetItemContent(self.ugc, self.handle, c.as_ptr()))? }
        Ok(self)
    }
}
```

`assert!` 一律改为返回值检查，路径错误、字符串含 NUL、API 失败全部走 `Result`。

#### 决策 3：接口指针运行时守卫 + 缓存

针对实证缺陷 #4 和 #8。

> **方案修正（见 0.7 节）**：此处原写 `NonNull + Result`，但那会破坏 23 个公开方法的签名。改为采用 **timeline 式 `disabled` 标记**——`src/timeline.rs:7` 已在本仓验证可行，零 API 破坏且同样在 release 下安全。下面是修正后的方案：

```rust
pub struct Friends<'a> {
    client: &'a Client,
    ptr: *mut sys::ISteamFriends,
    /// 对应 src/timeline.rs 已有的 disabled 模式
    unavailable: bool,
}

impl Friends<'_> {
    /// 每个方法入口统一守卫，替代 debug_assert!
    #[inline]
    fn guard(&self) -> bool {
        if self.unavailable {
            return false;   // 接口不可用：静默返回，不解引用
        }
        true
    }

    pub fn name(&self) -> String {
        if !self.guard() { return String::new(); }
        // 正常路径
        unsafe { ... }
    }
}
```

`unavailable` 在接口获取时一次性判定（`ptr.is_null()`），之后所有方法走同一守卫。相比 `Result` 方案，代价是「静默降级」的语义较弱；收益是 23 个公开签名零变更。

**保留为备选**：若将来某接口变成「可能不可用且调用方必须感知」，再对那一个接口单独升级为 `Result` 形式（不必全量改）：

```rust
pub struct Interfaces {
    friends: NonNull<sys::ISteamFriends>,
    // ...
}

impl Interfaces {
    fn get(&self) -> Result<...> {
        let ptr = unsafe { sys::SteamAPI_SteamFriends_v018() };
        NonNull::new(ptr).ok_or(SteamError::NotInitialized)?  // release 也判空
    }
}
```

#### 决策 4：`Client` 用 `Box<Inner>` 而非 `Arc<Inner>`

针对实证缺陷 #8。**注意：此项的理由已修正**——实测显示 Arc 克隆仅 8ns/次、占帧预算 0.0005%，**不构成性能问题**（见 0.5 节）。改用 `Box` 的真实理由是类型安全与 API 清晰度：

```rust
pub struct Client {
    inner: Box<Inner>,   // 独占所有权
}

impl Client {
    #[inline]
    pub fn friends(&self) -> Friends<'_> {
        Friends { client: self }   // 借用视图
    }
}
```

收益：
1. **类型安全**：接口对象受 `Client` 生命周期约束，编译期阻止 use-after-free
2. **可失败构造**：接口获取可返回 `Result`（指针为 null 时报错），替代 `debug_assert` + 裸指针解引用
3. **代码精简**：消除 42 处 `self.inner.clone()` 样板，同时消掉 clippy 的 `type_complexity` 警告

「省下两次原子操作」是附带效果，**不是采纳此决策的理由**。

---

## 3. API 设计

### 3.1 初始化

```rust
use supersteam::{Client, Config, InitError};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new()?;                      // 简洁路径

    let client = Client::with_config(                // 高级路径
        Config::builder()
            .app_id(1234567)
            .init_timeout(Duration::from_secs(30))
            .auto_relaunch(true)
            .build()
    )?;

    Ok(())
}
```

### 3.2 异步 API（对比：steamworks-rs 无 async，Steamworks.NET 无 Task）

```rust
use supersteam::{Client, Friends};

async fn friend_report(client: &Client) -> Result<String, Error> {
    let friends = client.friends();

    let list = friends.get_friends().await?;

    // 并行拉取所有好友详情，而非串行回调地狱
    let details = futures::future::join_all(
        list.iter().map(|id| friends.get_friend_details(*id))
    ).await;

    Ok(format!("{} friends, {} details", list.len(), details.len()))
}
```

异步由 `Future` 适配器实现：回调分发时唤醒对应 Waker，无需独立线程池。

### 3.3 构建器 + 可失败链

```rust
fn upload(client: &Client) -> Result<PublishedFileId, Error> {
    client.ugc()
        .start_item_update(app_id, file_id)?
        .title("My Mod")?
        .description("A cool mod")?
        .content_path(Path::new("./content"))?   // 每一步都可失败，不 panic
        .visibility(Visibility::Public)
        .submit()
        .await
}
```

对比现状（src/ugc.rs:974）：`canonicalize().unwrap()` 在路径不存在时直接 panic。

### 3.4 类型安全防混用

```rust
// SteamId / AppId 为独立 newtype，编译器阻止跨用
fn send(client: &Client, to: SteamId, msg: &str) -> Result<(), Error> { ... }

// send(client, app_id, "hi");   // 编译错误，而非运行时错乱
```

### 3.5 流式事件

```rust
let mut events = client.events().subscribe();
while let Some(event) = events.next().await {
    match event {
        Event::PersonaChanged(p) => log::info!("{} changed", p.name),
        Event::LobbyJoined(l) => on_lobby_join(l)?,
        _ => {}
    }
}
```

---

## 4. 性能优化策略

### 4.1 关于「消除原子操作」的更正

**此项目标已撤销。** 实测（见 0.5 节）显示 `Arc::clone` 仅 8ns/次，60 FPS 下每帧取 10 个接口占帧预算 0.000481%——**不构成性能问题**。

我最初把「37 个方法各执行一次 `Arc::clone`」解读为「高频调用路径会累积开销」，这个解读是错的：累积后的绝对值（每帧约 80ns）仍在噪声范围内。

`Box<Inner>` + 借用视图仍然采用，但理由改为**类型安全与 API 清晰度**（见决策 4），不是性能。把这一节标题从「消除原子操作」改为准确描述，是为了避免读者误以为此处有性能收益。

### 4.2 字符串处理：仓内已有优良做法，真缺口在写入侧

**先修正一个不完整的描述**。我此前写「steamworks-rs：`CString::new().unwrap()` —— 每次分配」，这漏掉了仓内已有的零拷贝读法。

核查结果（`src/` 共 14+ 处）：

| 方向 | 现状 | 评价 |
|------|------|------|
| **读**（Steam → Rust） | `CStr::from_ptr()` + `to_string_lossy()`，见 `src/friends.rs:64-68`、`src/app.rs:88`、`src/app.rs:104/117/134/155/170`、`src/input/origins.rs:23`（拆分前这个读法在该文件里只有一处，旧行号 126 早就漂了——拆分前那份文件里，这个读法在第 542 行） 等 | **已是零拷贝**，无中间分配，可直接借鉴 |
| **写**（Rust → Steam） | `CString::new(...).unwrap()`，77 处 panic 点 | **这是真缺口** |

典型的读法（`src/friends.rs:64-68`）：

```rust
pub fn name(&self) -> String {
    unsafe {
        let name = sys::SteamAPI_ISteamFriends_GetPersonaName(self.friends);
        let name = CStr::from_ptr(name);        // 零拷贝借用 Steam 侧缓冲区
        name.to_string_lossy().into_owned()     // 仅此一次分配，不可避免
    }
}
```

**结论**：SuperSteam 的字符串读路径可以直接沿用这套做法，不需要发明新东西。真正需要设计的是**写路径**——如何在「零 panic」前提下避免每次调用都堆分配：

```rust
pub(crate) struct StackString {
    buf: SmallVec<[u8; 64]>,
}
impl StackString {
    pub fn new(s: &str) -> Result<Self, SteamError> {
        let mut buf = SmallVec::new();
        buf.extend_from_slice(s.as_bytes());
        buf.push(0).ok_or(SteamError::InvalidParameter)?;  // 容量满 → 错误而非 panic
        Ok(Self { buf })
    }
}
```

注意仓内已有栈缓冲的**雏形**但未用在这条路径上：`src/app.rs:81/128/149` 用 `vec![0; 2048]` 做读出缓冲，`src/friends.rs:412/433/454` 用 `vec![0; 32*32*4]` 做头像缓冲。这些仍是堆分配（`vec!`），改为 `SmallVec` 或定长数组可消除，但优先级低——它们只在接口调用时发生，非热路径。

**方法论教训**：我最初把字符串处理整体描述为「每次都分配」，是因为只看了写入侧的 `CString::new`，没有先问「读取侧是怎么做的」。这与 #4 讨论中漏看 `timeline` 先例是同一个错误模式——**只盯着要修的地方，没先盘点仓内已有实现**。

### 4.3 回调分发上限

针对实证缺陷 #3：`apicall_result.resize(m_cubParam)` 接受任意大小。方案设上限并跳过：

```rust
const MAX_CALLBACK_SIZE: usize = 8 * 1024 * 1024;  // 8MB
if apicall.cub_param > MAX_CALLBACK_SIZE {
    log::error!("callback size {} exceeds limit", apicall.cub_param);
    continue;  // 跳过而非分配畸形大小
}
```

### 4.4 零拷贝数据暴露

沿用并强化 steamworks-rs 已有的 `data() -> &[u8]` 模式，用于网络消息与远程存储读取路径。

---

## 5. 安全设计

### 5.1 unsafe 隔离

现状：415 个 unsafe 块散布 20 个模块。

方案：所有 unsafe 收敛到 `supersteam-ffi` 单一 crate，通过宏生成。上层 crate 通过 `#![forbid(unsafe_code)]` 强制保证——这是可以用编译器验证的硬约束。

### 5.2 资源生命周期

```rust
pub struct Client { inner: Box<Inner> }

impl Drop for Inner {
    fn drop(&mut self) {
        // 析构前强制要求所有接口句柄已释放（编译期借用检查保证）
        unsafe { sys::SteamAPI_Shutdown() }
    }
}
```

对比 Steamworks.NET 用 finalizer 释放非托管内存（易泄漏且时机不确定），Rust 的确定性 Drop 是结构性优势。

### 5.3 空指针守卫

现状 `debug_assert!(!ptr.is_null())` 在 `--release` 下完全失效，直接 UB。

方案：所有 FFI 返回值做运行时判空，release 与 debug 行为一致。具体采用 timeline 式 `disabled` 标记（零 API 破坏，见 0.7 节与决策 3）；`NonNull::new(...).ok_or(...)` 保留为「调用方必须感知不可用」时的备选。

---

## 6. 功能覆盖

### 6.1 当前缺口（实证：宽口径 34 个访问器接口，封装 23 个，缺口 11）

> **本节数字是 2026-10-12 的历史快照**（写于补齐之前）。缺口闭合后的**复算尺见 6.1b**；
> 两节口径不同（本节=头文件里的接口访问器，6.1b=生成绑定里的 `SteamAPI_*_vNNN` 函数），
> 不要跨节拿数字互相减。

> **深挖修正（defect-07，双重纠错）**：原写「29 个接口，封装 14，缺失 15」——**分母和分子都错**：
> 1. 分母 29 用窄正则漏了 5 个 `_SteamAPI_` 中缀命名的接口（NetworkingMessages/Sockets/Utils + GameServer 侧 2 个），宽口径实为 **34**；
> 2. 分子 14 漏算了 GameServer 侧 6 个访问器封装（server.rs + networking 服务器变体），宽口径实为 **23**；
> 3. 缺失 15 中混入 3 个无访问器接口（AppTicket/GameCoordinator/DualSense），真缺口 **11**——且 **11 是口径不变量**（宽窄两口径缺口清单逐项相同）。
> 方法数为双口径：严=单行纯虚、宽=含跨行 `virtual`。

已封装（23）：
- Client 17：utils, matchmaking, matchmaking_servers, networking, apps, friends, input, user, user_stats, remote_play, remote_storage, screenshots, ugc, timeline, networking_messages, networking_sockets, networking_utils（= lib.rs 全部 getter）
- GameServer 6：GameServer, GameServerUGC(`server.rs:536`), GameServerNetworking, GameServerUtils, GameServerNetworkingMessages, GameServerNetworkingSockets

缺失（11）：
| 接口 | 虚方法数 | 重要性 | 备注 |
|------|---------|--------|------|
| ISteamInventory | 30 严/38 宽 | **必补（高）** | 内购/开箱核心；**有停滞已测实现 PR #205**，补齐成本低于预估 |
| ISteamHTTP | 25 | **必补（高）** | 自建后端通信、图片下载、Web API |
| ISteamHTMLSurface | 37 严/39 宽 | **锦上添花（中）** | 游戏内网页 overlay（嵌入商店/社区） |
| ISteamParentalSettings | 6 | **合规必需（高）** | 家庭家长控制，部分区域上线必需 |
| ISteamGameServer*（HTTP/Inventory/Stats） | 25/30/~10 | **GameServer 必需（中）** | UGC/Networking/Utils 已封装，只缺这三个 |
| ISteamParties | 12 | **锦上添花（中）** | Steam 原生日派对系统（可被 Matchmaking 绕开） |
| ISteamMusic | 9 | **可选（低）** | Steam 音乐播放器集成（极少游戏用） |
| ISteamVideo | 4 | **可选（低）** | Steam 视频播放器集成 |
| ISteamController | 34 | **已取代（低）** | **注意**：已被 ISteamInput（`src/input/` 九件，`type_surface` 复算 45 个 `pub fn`）完全取代，缺失是主动选择 |

**关键发现**：
- **ISteamController 不是真正的功能缺口**。头文件注释明言"已被 ISteamInput 取代"，Rust 侧用 Input 模块（`src/input/`，45 个 `pub fn`）覆盖其核心功能（GetActionSetHandle、GetDigital/AnalogActionData 等）。Steamworks.NET 包含它可能是为了兼容性。
- **真正的必补缺口**是 Inventory（内购）和 HTTP（自建后端），这两个缺了会阻塞大量游戏开发。
- **合规必需**是 ParentalSettings（家长控制），在某些区域上线必需。
- **GameServer 全套**是给服务器开发者用的，不做服务器则低。

注：Steamworks.NET 覆盖近全量（含上述冷门接口），这是它相对 steamworks-rs 的明确优势，SuperSteam 必须补齐。

### 6.1b 缺口闭合后的复算（2026-10-14）

上表写于补齐之前，现在缺口已经闭合，所以重量一次并把尺留在纸上。这次的尺不数头文件里的
接口，而是数**生成绑定里的访问器函数**——只有真的存在可调用符号，才谈得上"缺不缺"：

```bash
grep -ohE "SteamAPI_[A-Za-z]+_v[0-9]+" steamworks-sys/src/windows_bindings.rs | sort -u   # 29 个访问器
grep -ohE "SteamAPI_[A-Za-z]+_v[0-9]+" src/ -r | sort -u                                   # 28 个被调用
comm -23 /tmp/acc_all.txt /tmp/acc_used.txt                                                # 差集
```

差集只剩一项：`SteamAPI_SteamController_v008`。也就是说 6.1 表里的 11 项缺口中，除
ISteamController 外**全部已封装并可从 `Client`/`Server` 取到**；Controller 是主动不接
（ISteamInput 已取代它，头文件注释明言），属于选择而非遗漏。

**两条对 6.1 的实测更正**（不改上表历史数字，更正记在这里）：

1. 表里 `ISteamGameServer*（HTTP/Inventory/Stats）` 记成"只缺这三个"，是按 C++ 头文件的宏访问器算的。
   生成绑定里它们是三个**独立可调用函数**（`SteamAPI_SteamGameServerHTTP_v003`、
   `SteamAPI_SteamGameServerInventory_v003`、`SteamAPI_SteamGameServerStats_v001`）；补齐后
   `Server::http()` / `Server::inventory()` 直接复用客户端那两套包装类型，`Server::stats()` 用
   `src/server_stats.rs`。没有新增接口族，只有新增访问器调用。
2. 表里 ISteamHTMLSurface 的方法数（37 严/39 宽）成立，但**不含 `RunFrame`**：本 SDK 版本的
   `steamworks-sys/lib/steam/public/steam/isteamhtmlsurface.h` 里搜不到 `RunFrame`（整文件 39 处
   `virtual`，`grep -n RunFrame` 为空）。所以包装层不能照搬"每帧调用一次"的客户端惯例，
   `src/html_surface/`（今天是 `mod.rs`+`browser.rs`+`keys.rs`+`callbacks.rs` 四个文件）
   也没有发明这个方法：整目录 `grep -rn RunFrame src/html_surface/` 实测 0 命中，头文件同样 0 命中。

**这把尺的盲区要一并说清**（否则会误读成"覆盖率 28/34＝82%"）：访问器函数只有 29 个 ≠ 接口只有 29 个。
ISteamNetworkingMessages / ISteamNetworkingSockets / ISteamNetworkingUtils / ISteamClient 这几族没有
`SteamAPI_X_vNNN` 形式的访问器（绑定里是 `_SteamAPI_` 中缀或指针传递），它们的方法在 `src/` 里确实被调用，
但上面的正则看不见。所以 6.1 的"34 个接口"与本节的"29 个访问器"是两个分母，不能互相减。

### 6.1c 接口闭合 ≠ 方法闭合（方法级尺，2026-10-14）

缺口按"接口"数完了，按"方法"数还差得多。同一族函数名前缀，分母取绑定里声明的可调用数：

```bash
grep -ohE "SteamAPI_ISteam<Name>_[A-Za-z0-9_]+" steamworks-sys/src/windows_bindings.rs | sort -u | wc -l  # 绑定
grep -rhoE "SteamAPI_ISteam<Name>_[A-Za-z0-9_]+" src/ | sort -u | wc -l                                    # 已调
```

| 接口 | 已调 | 绑定 | 接口 | 已调 | 绑定 |
|------|-----:|-----:|------|-----:|-----:|
| ISteamNetworkingUtils |   14 |  41 | ISteamHTTP |   22 |  25 |
| ISteamController |    0 |  34 | ISteamMatchmakingPlayersResponse |    0 |   3 |
| ISteamClient |    0 |  33 | ISteamMatchmakingRulesResponse |    0 |   3 |
| ISteamRemoteStorage |   26 |  59 | ISteamMatchmakingServerListResponse |    0 |   3 |
| ISteamInput |   44 |  48 | ISteamMatchmakingServers |   14 |  17 |
| ISteamUser |   30 |  33 | ISteamMatchmakingPingResponse |    0 |   2 |
| ISteamNetworkingSockets |   28 |  47 | ISteamUGC |   97 |  99 |
| ISteamGameServer |   23 |  41 | ISteamNetworkingMessages |    6 |   6 |
| ISteamNetworking |    6 |  22 | ISteamVideo |    3 |   4 |
| ISteamFriends |   63 |  78 | ISteamGameServerStats |   10 |  10 |
| ISteamRemotePlay |    7 |  20 | ISteamHTMLSurface |   37 |  37 |
| ISteamUserStats |   31 |  44 | ISteamMusic |    9 |   9 |
| ISteamMatchmaking |   28 |  38 | ISteamParentalSettings |    6 |   6 |
| ISteamUtils |   32 |  37 | ISteamParties |   12 |  12 |
| ISteamApps |   29 |  33 | ISteamScreenshots |    9 |   9 |
| ISteamInventory |   34 |  38 | ISteamTimeline |   18 |  18 |
| ISteamNetworkingFakeUDPPort |    4 |   4 |  |  |  |

**总计 642 / 913**（33 个接口；2026-10-16 晚些时候再补 FakeIP 分配侧 26→28 与 `ISteamNetworkingMessages` 的 `CloseSessionWithUser`/`CloseChannelWithUser` 5→6（该接口首次到满），见上面的 FakeIP 段与 §6.1d；2026-10-15 先补 NetworkingUtils 诊断簇 3→14，再补 ISteamInput 手柄簇 18→35、
 glyph/origin 簇 35→43，再补 ISteamUser 14→17（五个账号状态谓词 + 时长管控状态 + 徽章等级 + 语音最佳采样率）
 与 ISteamInput 设备回调 43→44；2026-10-16 再补 ISteamUser 的四个 `STEAM_CALL_RESULT` 异步调用 17→21
 与语音簇五个 21→26，再补两个出参协议件 26→28、两个词表未定件 28→30，见 §6.1d）。这张表现在由 CI 的
`interface coverage stays at its registered ledger` 一步复算：尺子是
`scripts/ci/api_coverage.py`，基线是 `scripts/ci/baselines/api_coverage.txt`，
每个接口两条数（`declared` / `called`）**双向**判红 —— 少包一个函数、或 SDK 升级悄悄换了
可调用集合，都要人在提交信息里显式认过。上面那条 `grep` 只是同一件事的手工写法，
现在它不再是唯一口径。

四个 0 行要分两类说，别一概说成"不需要"：

- **确实不该包**：`ISteamController` —— `isteamcontroller.h:2` 原文 "The older ISteamController
  interface has been deprecated in favor of ISteamInput"，而 `ISteamInput` 本仓已包 18 个。
  `ISteamClient` —— 绑定给它生成 33 个入口，本仓一个都不调：接口指针一律来自扁平 C API 的
  `SteamAPI_Steam<Name>_vNNN()` 全局函数（`lib.rs` 24 处 + `server.rs` 8 处，均带 null 守卫）。
- **当时是真缺口，不是结构性豁免**：`ISteamMatchmaking{Ping,Players,Rules,ServerList}Response`
  （2~3 个入口各一）与 `ISteamNetworkingFakeUDPPort`（4 个）。判据是**内容尺而不是名字猜测**：
  `grep -rn "PlayersResponse\|PingResponse\|RulesResponse\|ServerListResponse" src/` 与
  `grep -rn "FakeUDPPort\|CreateFakeUDPPort" src/` 实测**零命中** —— 本仓连这些名字都没出现过，
  所以它们是没做，不是"做了但不占这些入口"的误读。
  **2026-10-16 复算**：`ISteamNetworkingFakeUDPPort` 四个入口连同工厂已全部封进
  `src/networking_sockets/fake_udp.rs`（`FakeUDPPort` 4/4、`ISteamNetworkingSockets` 25→26），
  上面那句 `grep` 从今天起不再零命中，所以把它标成"当时"。四个 `*Response` 仍读 0，原因见下段
  ——那是接收侧已实现、台账只数"本仓主动调用"的口径问题，不是缺口。
  需要区分的是：`matchmaking_servers.rs` 确实**实现了**这四类服务端回调 ——
  `matchmaking_servers_callback!`（该文件第 8 行）在 148 / 449 / 467 / 480 行以
  `ServerList` / `Ping` / `PlayerDetails` / `ServerRules` 实例化，手写 `extern "C"` vtable
  交给 Steam 回调进来。但那套实现**一次都没有调用**绑定给这四个接口生成的入口函数，
  所以台账读 0：这里 0 的含义是"入口没被调"，不是"接口不存在于本仓"。

FakeIP 那一族四个入口都在 `isteamnetworkingsockets.h`。**2026-10-16 补掉两个**：
`begin_async_request_fake_ip`（h:862）与 `create_listen_socket_p2p_fake_ip`（h:878），落在
`src/networking_sockets/fake_ip.rs`，`ISteamNetworkingSockets.called` 26→28。这一对能在读不到完成
消息的情况下封，靠的是头文件自己那句 h:875-877「you do not need to wait for the request to
complete」；`bool` 不改成 `Result` 是因为 h:835-836 把 `false` 定义成「已有一个请求在跑」，两个答案都
是正常状态。**余下三个阻碍各不相同**：`GetFakeIP`（h:867）与
`ISteamNetworkingUtils::SetGlobalCallback_FakeIPResult` 堵在生成层——`SteamNetworkingFakeIPResult_t`
被绑成空壳 `_unused: [u8; 0]`（`steamworks-sys/src/windows_bindings.rs:17499-17501`，真定义在
`steamnetworkingfakeip.h:93`），要动的是 bindgen 输入清单与四个重生成文件，属于生成层的决定；
`GetRemoteFakeIPForConnection`（h:895）技术上可达（`SteamNetworkingIPAddr` 绑定具体，
`From<sys::SteamNetworkingIPAddr> for SteamIpAddr` 已在
`src/networking_types/addresses.rs:183-187`），但本仓所有面向地址的公开 API 都说
`std::net::SocketAddr`、`SteamIpAddr` 保持 crate 内部，所以它是「FakeIP 要不要进公开类型面」的设计
决策，登记而不是猜。`fake_udp.rs` 里 `create_fake_udp_port` 的文档指回这一段，就是为了让"票从哪来"
这条边界有人看见而不是被假装跳过。

这张表的单位是**绑定里的可调用名字**（`grep -c "pub fn SteamAPI_ISteamUGC_"` 实测 99），不是头文件里的
方法数：`CreateQueryAllUGCRequest` 和 `CreateQueryUGCDetailsRequest` 各有重载，绑定给它们加了
`…RequestPage` / `…RequestCursor` 后缀，所以 99 个名字只对应 97 个不同的方法。UGC 补到 **97/99** 之后
剩下两条，各有明确去处：`RequestUGCDetails`（头文件原文 "DEPRECATED - Use CreateQueryUGCDetailsRequest
call above instead!"）与 `GetAppDependencies`（多次答案，见 6.1e）。
判"已调"的单位两边都是**绑定名**，所以精确相等就够了；但把**头文件名**拿去和绑定名精确相等是错的 ——
早先一把这样的尺子把 `CreateQueryAllUGCRequest` 报成从没调过，实际它由
`…_RequestPage` / `…_RequestCursor` 两个绑定入口调用。**头文件名 ≠ 绑定名**，这条尺子因此改成
绑定名对绑定名，并顺带把重载数（99 个名字 / 97 个方法）写在上面。

`ISteamScreenshots` 也在 2026-10-14 补到 **9/9**：新增 `write_screenshot`（原始 RGB 缓冲，字节数按
Steam 的 u32 口径校验，超限判 `InvalidParameter` 而不是截断；空缓冲给对齐悬垂指针而非 null）、
`add_vr_screenshot_to_library`（6 种 `VRScreenshotKind` 各自映射到独立 Steam 变体）、
`set_location` / `tag_user` / `tag_published_file`。两个测试同样做过变异验证
（把守卫改成 `len as u32`、把 `StereoPanorama` 塌成 `Mono`，各自把对应测试打红）。

`ISteamTimeline` 一行 2026-10-14 补到 **18/18**（整接口闭合）。补的时候发现并修掉一个**真参数顺序缺陷**：
`add_timeline_event(icon, title, description, ..)` 原来按 `(icon, title, description)` 传给
`AddRangeTimelineEvent`，而绑定签名是 `(pchTitle, pchDescription, pchIcon, ..)` ——
标题位收到图标名、图标位收到描述文本。Steam 头文件与 `windows_bindings.rs` 双侧核过才动手。
另外该函数原先把 `AddRangeTimelineEvent` 返回的 `TimelineEventHandle_t` 直接丢弃，
所以事后无法 Update/End/Remove；新版另开 `add_instantaneous_event` / `start_range_event` 等把句柄返回出来，
旧签名不动（不改已发布形态）。两个 `Does*RecordingExist` 是异步的，配了
`TimelineEventRecordingExists` / `TimelineGamePhaseRecordingExists` 两个回调。

`ISteamRemoteStorage` 一行 2026-10-14 由本轮补齐一部分：新增 `FileWrite` / `FileRead` / `GetQuota` /
`GetLocalFileChangeCount` + `GetLocalFileChange` / `BeginFileWriteBatch` + `EndFileWriteBatch`
（19→26，逐个名字复核过：绑定的 59 个里 `src/` 现在调到 26 个）。
**该接口剩下的 33 个未调方法绝大多数是退役的旧 Workshop 面**（`PublishWorkshopFile`、
`UpdatePublishedFile*`、`UGCDownload`、`GetCachedUGC*`、`Enumerate*`、投票类）——它们的替代者是
`ISteamUGC`（已封 60 个方法），**照搬过来会把同一件事实现两遍**，所以不列为待补。
`ISteamInput`（48−18）与 `ISteamTimeline`（18−4）是下一批候选。

### 6.1d 补口的取舍登记（2026-10-14，避免"没写"被误读成"漏了"）

`ISteamUtils` 补到 32/37 之后，剩下 5 个**逐个对着 `isteamutils.h` 判过**，不是漏项：

| 未封方法 | 判据（读头文件原文，不靠猜） |
|---|---|
| `GetAPICallFailureReason` | 参数是裸 `SteamAPICall_t`；本仓的回调派发链已经把失败映射成 `SteamError`，**开放裸句柄只加不安全面、不加新能力** |
| `CheckFileSignature` | DRM 向、异步且有独立回调结构，游戏侧极少用 |
| `GetConnectedUniverse` | 客户端连的是哪个 universe（public/beta…），诊断类 |
| `SetGameLauncherMode` | 头文件写明是给"没有手柄支持的启动器"用（Steam Input 转键鼠），不是游戏态 |
| `StartVRDashboard` | 头文件："ask SteamUI to create and render its OpenVR dashboard"，属客户端工具 |

`InitFilterText` / `FilterText` 已在同一轮补上（`init_text_filter` / `filter_text`）。
**这里留一条判错记录**：我最初打算以"已被 `ISteamTextFiltering` 取代"为由跳过这对函数，
但本绑定里 `grep -c SteamAPI_ISteamTextFiltering` = **0** —— 那套新接口在这份 SDK 绑定中根本不存在，
`FilterText` 是唯一的文本过滤通路。判"某 API 已被取代"必须先查绑定里有没有那个替代品，
不能按 Steamworks 的版本常识直接推。输出侧按头文件要求分配 `strlen(input)+1`，
回读时截到第一个 NUL（不依赖 Steam 一定补终止符），负数返回按失败处理而不是"改了 0 个字符"。

`ISteamFriends` 2026-10-14 补到 **36/78**（本地状态面）：`persona_state`、`friend_relationship`、
`friend_steam_level`、`rich_presence_keys`、`set_in_game_voice_speaking`、`unread_priority_chat_count`、
`set_listen_for_friends_messages`、`open_remote_play_together_invite_dialog`。
**顺带修掉一个可触发的 panic**：`Friend::state()` 原先以 `_ => unreachable!()` 收尾，而
`EPersonaState` 有 `k_EPersonaStateMax = 8`（Steam 对"这份绑定叫不出名字的状态"就这么回），
所以那不是不可达。现在映射抽成 `persona_state_from`，未知名落到**新增的 `FriendState::Unknown`**；
`FriendRelationship` 同理把 `Suggested_DEPRECATED` 与 `Max` 落到 `Unknown` 而不是伪装成活关系。
⚠ 对下游这是**给公共枚举加变体**（严格说破坏穷尽 match），故 CHANGELOG 单列披露。
另：同批次补上**公会（clan）同步面**（27→47 里的后 11 个调用）：`clan_count`/`clans`、
`clan_name`/`clan_tag`（Steam 未缓存时返回 `None` 而不是空字符串——"没数据"和"没名字"是两回事）、
`clan_owner`、`clan_officer_count`/`clan_officers`、`is_clan_public`/`is_clan_official_game_group`、
`clan_chat_member_count`、`is_clan_chat_admin`。

同日再补**公会/跟随的异步组**（47→53）：`request_clan_officer_list`（回应 `ClanOfficerListResponse`，
它到之前 `clan_officer_count`/`clan_officers` 读不到有意义值 —— 这条前提写在方法上）、
`download_clan_activity_counts`（只回 Steam 自己的 success 位，数字仍要用 `clan_activity` 回读，
所以不假装响应带数据）、`clan_activity`（缓存值，`None` = 还没数据）、`follower_count`、`is_following`，
外加 `request_friend_rich_presence` + `FriendRichPresenceUpdate` 回调 —— 没有这一下，
`rich_presence_keys`/`rich_presence` 往往读到空。
判据记录：`IsFollowing` 与 `GetFollowerCount` **返回 `SteamAPICall_t`**，不是头文件直觉上的 bool/int，
所以走 `register_call_result` 而不是同步返回。
`ISteamApps` 一行 2026-10-14 补到 **29/33**：DLC（数量、逐项列表含 `available` 归属位、安装/卸载、下载进度）、
家庭共享借用位、`timed_trial`（**Steam 的 bool 才决定"是不是试用"，秒数 0 仍然是试用**，映射抽成
`timed_trial_from` 以便钉住）、最早购买时间（0 → `None`）、`mark_content_corrupt`、
beta 分支（数量三元组、逐项 `BetaInfo`、`set_active_beta` 名字含内嵌 NUL 直接判 `InvalidParameter`）。
**余下 4 个的判据**：`RequestAllProofOfPurchaseKeys` / `RequestAppProofOfPurchaseKey` 是
传统 CD-key DRM（`k_cubAppProofOfPurchaseKeyMax`，现代发行不用）；`SetDlcContext` 只影响后续调用的
上下文，属边角；`GetInstalledDepots` 是数组出参（内容工具向，待补）。
`GetFileDetails` 已于同日补上：走 `register_call_result`，答案以
`Result<FileDetails, SteamError>` 交给回调（Steam 侧失败不会伪装成"大小为 0 的文件"），
20 字节 SHA-1 另给 `sha1_hex()`（40 个小写十六进制字符，实测变异：改成大写即判红）。

同理两条硬事实登记在此免得反复重查：
① 本 SDK 的 `isteamutils.h` 里两处 `Deprecated` 都是 `STEAM_PRIVATE_API`（`GetCSERIPPort`、`RunFrame`），**不在公开面**，所以不存在"漏封退役方法"；
② `FilterText` 与 `ISteamTextFiltering` 的关系要判之前先查绑定 —— 我最初按"已被取代"想跳过它，
是 `grep -c SteamAPI_ISteamTextFiltering = 0` 把这条想当然否掉的。

`ISteamFriends` 同日补**聊天簇**（53→63；复算尺剩 15 个未调：好友分组 5
`GetFriendsGroupCount`/`IDByIndex`/`Name`/`MembersCount`/`MembersList`、来源服务器 3
`IsUserInSource`/`GetFriendCountFromSource`/`GetFriendFromSourceByIndex`、跟随列表 1
`EnumerateFollowingList`、历史昵称 1 `GetFriendPersonaNameHistory`、装备外观 4
`RequestEquippedProfileItems`/`BHasEquippedProfileItem`/`GetProfileItemPropertyString`/`Uint`、
协议注册 1 `RegisterProtocolInOverlayBrowser`。**这 15 个只列了清单、还没逐个判据**，
留给下一轮按 6.1d 的写法补）：
`friend_message` / `reply_to_friend_message`（把 `set_listen_for_friends_messages` 那扇门补齐 ——
上一轮我只交了"打开拦截"，文档明写"打开后 Steam 不再自己显示消息"却没有任何接收入口，
等于我亲手留了一个会把玩家消息吞掉的半门）、`send_clan_chat_message` / `clan_chat_message`、
`join_clan_chat_room`（`register_call_result` + `ClanChatRoomJoined`）/ `leave_clan_chat_room` /
`clan_chat_members`、覆盖窗三件套 `is/open/close_clan_chat_window_in_steam`，
回调 `FriendChatMessageReceived` / `ClanChatMessageReceived` / `UnreadChatMessagesChanged`。
三条口径：
① `EChatEntryType` **不物化成 sys 枚举**——Windows 绑它是 `repr(i32)`、其余平台 `repr(u32)`，
且两边都 `#[non_exhaustive]`，所以 out 参数用 4 字节裸 `i32` 接住再映射；
映射函数 `chat_entry_type_from` 用 `u8::try_from` 而不是 `as u8`，
变异对照：改成 `raw as u8` 后 257 会伪装成 `ChatMsg`，测试判红。
② 接收缓冲长度经 `chat_buffer_len` 夹到 `[1, MAX_CHAT_MESSAGE_BYTES]`，
这样 `capacity as i32` 是无损转换（变异：去掉夹取，`usize::MAX` 那条断言判红）；
Steam 报回的字节数截到容量、文本截到第一个 NUL、UTF-8 非法走 lossy（不 panic）。
③ 发送侧内嵌 NUL 判 `Err(SteamError::InvalidParameter)`，与 Steam 自己的拒绝 `Ok(false)` 分两级。

`ISteamUser` 现在 30/33（2026-10-16 依次补进：四个 `STEAM_CALL_RESULT` 异步调用走
`register_call_result`、语音簇五个进 `src/user/voice.rs`、两个出参协议件进 `src/user/buffers.rs`、
最后两个"词表未定"件进 `src/user/passthrough.rs`，见下面第 4、6、7 段）。**剩下 3 个全部是
"判过之后有意不封"，没有一个是漏项**，判据一律指到 `isteamuser.h` 行号：

| 未封方法 | 判据 / 前置条件 |
|---|---|
| `GetHSteamUser`（h:24） | 头文件 h:22-23 明写 "this is only used internally by the API, and by a few select interfaces that support multi-user"。把内部句柄开放给游戏代码只加不安全面、不加能力，**有意不封** |
| `InitiateGameConnection_DEPRECATED`（h:53） | h:51 "DEPRECATED! This function will be removed from the SDK in an upcoming version."，**有意不封**（与本仓 `RequestUGCDetails` 同一先例） |
| `TerminateGameConnection_DEPRECATED`（h:60） | h:58 同上，**有意不封** |

同一张表原来的第 8 行（`RequestStoreAuthURL` h:194 / `RequestEncryptedAppTicket` h:165 /
`GetMarketEligibility` h:209 / `GetDurationControl` h:213）**本轮已封**，登记在这里是为了留下判据：
四个都返回 `SteamAPICall_t` 且带 `STEAM_CALL_RESULT(...)`，所以实现是"发调用→用 `register_call_result`
按句柄登记一次性回调"，不是单行转发。落地时按该判据各自补齐的东西：
`request_store_auth_url` 把 512 字节定长 `m_szURL` 交给 `steam_out_string`（截在缓冲内第一个 NUL，
不越过 Steam 拥有的字节），入站 URL 内嵌 NUL 直接 `Err(InvalidParameter)` 且 `cb` 在返回前触发；
`market_eligibility` 原样转发 `EMarketNotAllowedReasonFlags`（bindgen 给的是 `repr(transparent)` newtype
加 `k_E…None/TemporaryFailure/AccountDisabled/AccountLockedDown/AccountLimited`＝0/1/2/4/8，
是**会组合的位**，镜像成 Rust 枚举要么丢位要么把两个原因说成一个），时间戳保留 Steam 的
`RTime32` 语义（0＝"没说"，不是 1970）；`duration_control` 的 `m_progress` 与 `m_notification`
是两个不同的词表（`steamclientpublic.h` 里 `EDurationControlProgress` 把 0..2 定义为进度、3..5 定义为
"该退出了"），因此没有合并成一个三选枚举，两边各带一个 `Unknown` 兜底臂；
`request_encrypted_app_ticket` 只交付"就绪"信号，票本体走 `GetEncryptedAppTicket` 的出参协议——
该行本轮也已被封（见下面第 6 段），空 `app_data` 也要给 Steam 一个非空指针，沿用 `write_file` 的
`NonNull::<u8>::dangling()` 写法。`m_eResult` 一律经 `to_steam_result`，IO 失败回 `IOFailure`。
两个兜底臂今天用现有头文件构造不出来（给 C 枚举造未声明的判别式是 UB，不是复现；实测这两个枚举
Windows 绑成 `repr(i32)`、其余三个平台绑成 `repr(u32)`，两个方向结论一样），
按既有纪律**只登记不写假测试**，兜底由 `no_panic_on_unknown_enum` 账本看着。

同一张表原来的语音三行（`StartVoiceRecording` h:72、`StopVoiceRecording` h:77、`GetAvailableVoice`
h:83、`GetVoice` h:106、`DecompressVoice` h:114）**本轮已封**，落地在 `src/user/voice.rs`。
原来登记的前置条件——"要先定缓冲谁分配多大、Steam 写了几个字节、缓冲不足回什么"——是照签名逐条定的：

1. deprecated 的 uncompressed 参数**一律不暴露**，永远传 `false`/null/0。头文件先说 most applications
   该把它们留成 NULL/zero（h:96-97），再说取未压缩数据会让 `GetVoice` **丢弃剩余的压缩数据**
   （h:90-92）：把它们开进 API 就是给调用方一条静默丢数据的口子。
2. 读路径每帧都要跑（"at least once per frame, and preferably every few milliseconds"，h:100-102），
   所以底层形状是**调用方给缓冲**（`read_compressed_voice_into` / `decode_voice_into`），
   `Vec` 版（`read_compressed_voice` / `decode_voice`）建在同一把尺上，不是两套逻辑。
3. **两条生长循环故意不一样**，因为头文件只给了一条的数量答案：`DecompressVoice` 明写缓冲不够时
   `*nBytesWritten` 就是所需大小、建议从 20kb 起步（h:111-113），那一路把报回的数当**下限**
   （`grow_for_decode`）；`GetVoice` 没有这个承诺，所以捕获那一路只能翻倍重试且次数封顶
   （`grow_for_capture` + `CAPTURE_ATTEMPTS`），超过 `MAX_VOICE_BYTES`（1 MiB）就**报
   `BufferTooSmall` 而不是继续分配**。
4. 告诉 Steam 的长度**只降不升**地夹到 `u32`（`voice_len`）——反方向是截断，会让它以为缓冲比实际更大；
   Steam 报回的字节数只信到"这把缓冲装得下"为止（`written_bytes`）。这两条各配一只变异实测判红。
5. `StopVoiceRecording` 之后采集不会立刻停（h:74-76 "the system will keep recording for a little
   bit"），所以它不返回"已经停了"，由读路径继续报 `NotRecording` 直到排空。
`VoiceError` 逐个映射 `EVoiceResult` 的 9 个非 OK 码（`steamclientpublic.h:151-163`，判别式 0..9，
那份声明本身不带注释，所以每个变体的中文说明读的是名字而不是头文件原文；有原文可引的地方都在
方法文档里引了），
`Unknown` 兜底臂同上：今天构造不出来，只登记不写假测试。`decode_voice` 故意不复用
`decode_voice_into`——后者会丢弃字节数，而缓冲不够时那个数正是下一次分配的唯一输入。

表里原来的 `GetUserDataFolder`（h:69）与 `GetEncryptedAppTicket`（h:173）两行**本轮已封**，落在
`src/user/buffers.rs`。这两件当初登记的理由是同一句"调用方缓冲 + 长度语义未定"，而头文件只把
**票**那一件说清楚了（h:167-172：返回 false 时 `*pcbTicket` 就是所需大小；传
`pTicket=NULL, cbMaxTicket=0` 探长度，"if a ticket is available, *pcbTicket will contain the size
needed, otherwise it will be zero"），`GetUserDataFolder` 的 `bool` **头文件从头到尾没解释 false
是什么意思**。于是两件的契约分别这么定：

1. `encrypted_app_ticket()` 照头文件走两遍：NULL/0 探一次 → 按报回的数分配 → 再取一次。
   探到 0 就是"还没票"→ `InvalidState`（这既不是调用方的错也不该重试到永远）；报回的数超过
   `MAX_TICKET_BYTES`（1 MiB）→ `InvalidParameter` 而**不是**照那个数分配；第二遍仍然 false →
   `InvalidState`，写清楚"协议没收敛是 Steam 那边的事"。取回的字节数照语音簇同一把尺夹到缓冲
   容量（共用 `voice::written_bytes`，不再写第二份）。
2. `user_data_folder()` 的失败检测**不依赖 Steam 那个没定义的 bool**：本仓给一个 32 KiB 缓冲
   （头文件自己举的例子 `C:\Program Files\Steam\userdata\<SteamID>\<AppID>\local` 远短于此，
   32 KiB 也已是 Windows 260 字符路径上限的百倍），若整块缓冲被填满而**内部没有 NUL**，就判定被
   截断并返回错误——因为截出来的字符串仍然"看着像一条路径"，静默交给调用方会让游戏把存档写到
   它没选的地方。Steam 回 false 也一律报 `InvalidState`，不替它编造含义。
3. `cubBuffer` 是 `int`，所以长度先过 `i32::try_from`；反方向（截断）会让 Steam 以为缓冲比实际更大。
4. 路径按本仓读定长缓冲的既有口径 lossy 解码（`steam_out_string`），并**在文档里写明**不可解字节
   会变成 U+FFFD、因此不保证字节级往返——而不是让调用方以为拿到的是原始字节。

4 个测试、4 只变异（去掉 NUL 守卫、把 `int` 长度改成截断、把探到 0 当成大小、去掉票上限），
每只都判到对应测试红、文件逐字节还原。

**第 7 段：`AdvertiseGame`（h:159）与 `TrackAppUsageEvent`（h:65）本轮已封**，落在
`src/user/passthrough.rs`，`ISteamUser` 因此到 30/33。这两件原来卡在"参数词表没定义"，处理办法
是**照头文件与本仓既有口径把假设说出口，而不是替它造一个词表**：

- `advertise_game(SteamId, Ipv4Addr, u16)`：h:156-158 对 `uint32 unIPServer` 只说 "the IP address
  of the game server"，**从没讲字节序**。所以取的是本仓已经在两头用过、且能原样往返的那个约定：
  出站 `MatchmakingServers::ping_server` 传 `ip.into()`（`src/matchmaking_servers.rs:447-458`），
  入站服务器列表把 Steam 自己给的 `m_unIP` 用 `Ipv4Addr::from` 读回
  （`src/matchmaking_servers.rs:213`）。实测这台小端机上 `u32::from(a) == a.to_bits()`，
  即"首段octet 作高位"（`192.168.1.1` = `0xC0A80101`）——选它不是拼写二选一，而是 Steam 回给你的
  就是这个形状。这个 Steam 调用返回 `void`，所以封它也不产生一条能被观测的成功/失败；要不同词序的
  人可以直取 `sys::SteamAPI_ISteamUser_AdvertiseGame`，**故意的错比凭猜的 wrapper 好**。
- `track_app_usage_event(GameId, i32, &str) -> Result<(), SteamError>`：`eAppUsageEvent` 在这份绑定里
  是裸 `int`、**SDK 没有对应枚举**（h:63-64 整条注释只有 "used by only a few games to track usage
  events"，且挂在头文件自己的 `// Legacy functions` 标题下）。因此**不**造一个 Rust 枚举——那会是一
  张本仓无法核对的名字表，跟写方向占位值同一类错误；数字原样转发，唯一能失败的地方是入站字符串的
  NUL，走 `c_string_arg` → `InvalidParameter`（不 `unwrap`、不静默截断）。

两件都**没有配测试**，理由是既有的那条纪律而不是省事：`advertise_game` 是一行转发、无可分叉的判据，
`track_app_usage_event` 唯一的分支要在有 `Client` 时才走得动，编一个假客户端去"测"它只会写出一只
杀不死的断言（同 §6.1d 上面那五个账号谓词的处置）。真正的锁是覆盖账本：少调一个 FFI 就让
`ISteamUser.called` 掉数，而那条判定双向判红。


**这里也记一条判据教训**：上面这些"未封"里没有一个是用"名字看起来像旧的/内部用的"当理由的——
`GetHSteamUser` 的不封依据是头文件那句话，两个 `_DEPRECATED` 的依据是 h:51/h:58 的原文，
最后两个"参数表示未定"的也已在 `src/user/passthrough.rs` 封完（见第 7 段），所以这张表现在
只剩 3 行、全是**有意不封**，没有一条是漏项。
判"要不要封"读签名和头文件，不读命名。

`ISteamInput` 补到 44/48（2026-10-15 加 `EnableDeviceCallbacks` + `DeviceConnected` /
`DeviceDisconnected` 两个载荷，见 §6.1 下方实现）之后，剩下 4 个同样逐个判过：

| 未封方法 | 判据（读绑定与头文件，不靠猜） |
|---|---|
| `SetDualSenseTriggerEffect`（`isteaminput.h:1020`） | 参数是 `const ScePadTriggerEffectParam *`，而**绑定里这个类型是空壳**：`windows_bindings.rs:11771` 生成的是 `pub struct ScePadTriggerEffectParam { _unused: [u8; 0] }`（SCE pad SDK 的定义不在随仓头文件里）。没有字段就无法安全构造一个 Steam 要读的结构体，自己复刻那套力反馈联合体布局属于凭猜定布局，传错是 UB。要封得先补上那份定义 |
| `EnableActionEventCallbacks`（`isteaminput.h:874`） | 参数是 `SteamInputActionEventCallbackPointer = Option<unsafe extern "C" fn(*mut SteamInputActionEvent_t)>`（绑定 `windows_bindings.rs:11774`）——裸 C 函数指针回调，头文件 h:872 明说它是"比标准 Steam 回调更低延迟"且"一次只支持一个回调"。要做就得加 extern "C" 蹦床 + 闭包注册表 + 单槽所有权规则，是另一件事，不是加一行转发 |
| `Legacy_TriggerHapticPulse`（h:968）/ `Legacy_TriggerRepeatedHapticPulse`（h:972） | 头文件自己给了替代路径：h:966 "if you are approximating rumble you may want to use TriggerVibration instead"。按上一条那条判据教训，先核实替代品真在绑定与本仓里：`SteamAPI_ISteamInput_TriggerVibration` 与 `TriggerSimpleHapticEvent` 都已封（`src/input/effects.rs` 的 `trigger_vibration` / `trigger_simple_haptic_event`），且这两个 Legacy 只作用于 Steam Controller。所以是**已被覆盖**，不是漏项 |

顺带修正本文早前的一处复现行号：缺陷 #3 那句"镜像 `src/lib.rs:170` 的 `resize` 行为"里的
`resize` 现在在 `src/lib.rs:308`（拆分后行号漂移，本轮按内容重测；原句描述的行为本身没变）。

### 6.1e `ISteamUGC` 39 个未调方法的分类（2026-10-14 实测登记，Tier 2 开工前的地图）

分母先修正：`windows_bindings.rs` 里 `SteamAPI_ISteamUGC_*` 有 99 个名字、`src/` 调到 60 个，
但对着 `isteamugc.h` 逐个核过，这 99 不是 99 件不同的事：

- `CreateQueryAllUGCRequestPage` 与 `CreateQueryAllUGCRequestCursor` 是 C++ 同名
  `CreateQueryAllUGCRequest` 的**两个重载**，靠 `STEAM_FLAT_NAME` 才有两个平面名 —— 都真、都该封；
  本页只封了 Page 那一支。
- `GetQueryFirstUGCKeyValueTag` 与已封的 `GetQueryUGCKeyValueTag`（按索引取一对 KV）也是同一
  `STEAM_FLAT_NAME` 现象下的**两个重载**（前者按 key 取第一个 value），不是重复。
- `RequestUGCDetails` 头文件原文写 `// DEPRECATED - Use CreateQueryUGCDetailsRequest call above instead!`
  ⇒ **登记为不补**，理由来自头文件而非推测。
- 其余 36 个在头文件里都找不到 `Deprecated` 字样，属现役面。

按"要不要新回调类型"分四批，避免一批里混着同步 getter 和异步答案：

| 批次 | 内容 | 方法 |
|---|---|---|
| 2a 查询侧 —— **已交，60→73** | `tag`/`tag_display_name`/`tag_count`、`additional_preview*`（新增 `PreviewKind` 六变体 + `Unknown`）、`first_key_value_tag`、`supported_game_version*`、`add_required_tag_group`、`set_return_playtime_stats`、两个时间区间、`query_all_with_cursor` | 13 ✓ |
| 2b 更新侧 —— **已交** | `add_preview_file`（按值收 `EItemPreviewType`，`Unknown` 直接拒发）、`add_preview_video`、`update_preview_file/video`、`remove_preview`、`set_required_game_versions`（空串=该端不限，头文件原文）、`set_admin_query`、`set_allow_legacy_upload`、`show_workshop_eula` | 9 ✓ |
| 2c 本地缓存/数组出参 —— **已交** | `downloaded_item_count`、`downloaded_items`、`mark_downloaded_item_as_unused`、`set_items_disabled_locally`、`set_subscriptions_load_order`、`user_content_descriptor_preferences` | 6 ✓ |
| 2d 异步答案 —— **已交 9 个** | `Add/RemoveAppDependency`、`Add/RemoveDependency`、`Add/RemoveItemToFavorites`、`Get/SetUserItemVote`、`GetWorkshopEULAStatus` | 10 中的 9 |
| 2d 余下 | `GetAppDependencies`：**页面类型已进流**（`GetAppDependenciesResult`），一次性包装**有意不做** | 1 |
| 不补 | `RequestUGCDetails`（头文件明写 DEPRECATED，替代者 `CreateQueryUGCDetailsRequest` 已封） | 1 |

2b/2c 补的时候顺手修掉两条**真缺陷**（都不是"顺手美化"）：
① `QueryResults::content_descriptor` 把 Steam 报回的计数直接当切片上界用
（`&descriptors[..num_descriptors]`，数组容量 10）——计数只要大于容量就 **panic**；
现在过 `clamped_entries` 夹到容量（变异：去掉 `min` → `steam_reported_counts_are_clamped_to_the_buffer` 判红）。
② 同一处用 `unsafe { std::mem::zeroed() }` 造 `[EUGCContentDescriptorID; 10]`，
而 `EUGCContentDescriptorID` 的第一个码是 **1**（实测绑定），全零不是这个枚举允许的值，
按值构造即 UB；改为用一个有效变体初始化。新的 `user_content_descriptor_preferences`
同样按有效变体起手、同样夹计数，并把容量钉成 `const _: () = assert!(... > 5)`——
数组缩到装不下 Steam 现有 5 个码时**编译不过**，而不是测试变红（对照 M6 实测为 BUILD-FAIL）。

2a 的三条实测口径：
① **缓冲长度一律取 Steam 自己的常量**，不用我拍的数：单个 tag 用 `k_cchTagListMax = 1025`、
preview 的 URL 用 `k_cchPublishedFileURLMax = 256`、原始文件名用 `k_cchFilenameMax = 260`
（三者都在四平台绑定里存在）；只有 KV 值与 branch 名 Steam 没给常量，沿用本文件一直在用的 256 并写成命名常量。
② **回读不信任 Steam 的终止符**：`steam_out_string` 在"我们自己拥有的这块缓冲"里截到第一个 NUL，
答案填满缓冲时也不会读到缓冲区外（变异：把 `unwrap_or(buffer.len())` 改成 `unwrap_or(0)` 判红）。
③ `EItemPreviewType` 同 `EChatEntryType` 一样 **Windows `repr(i32)`、其余 `repr(u32)` 且两侧 `#[non_exhaustive]`**
（实测四份绑定），所以 out 参数按裸 `i32` 接住再映射，`ReservedMax = 255` 落 `PreviewKind::Unknown`。

顺带把游标分页**闭环**：`SteamUGCQueryCompleted_t.m_rgchNextCursor`（四平台绑定都是 `[c_char; 256]`，
头文件原文 "If a paging cursor was used, then this will be the next cursor to get the next result set"）
以前被丢掉，现在 `QueryResults::next_cursor()` 把它读出来，空串 = 没有下一页（`then_some`，变异：改成恒 `Some` 判红）。
`add_required_tag_group` 一个 tag 带内嵌 NUL 就**整组不提交**（抽成 `tag_group_strings` 才测得到这条；
变异成"静默丢掉坏字节"判红）。

2d 的形状问题按"一次调用 = 一个答案"这条界线切开处理：

- **9 个一问一答的先交**（`src/ugc/answers.rs`）。它们的 `STEAM_CALL_RESULT` 都是一次性的，
  `register_call_result`（`FnOnce`，按 `SteamAPICall_t` 只触发一次）正是它们的形状，
  所以统一走 `answered(io_error, v.m_eResult, || 载荷)`：IO 失败 → `IOFailure`，Steam 拒绝 → 它自己的
  `SteamError`，其余 → 载荷。8 个回调全部进了 `CallbackResult`（不是只挂 `register_call_result`），
  这样流式 API 也收得到。
- **`GetAppDependencies` 的一次性包装有意不做，但它的页面已经能拿到**。头文件原文是
  "note that whatever callback you register for `GetAppDependenciesResult_t` may be called multiple
  times until all app dependencies have been returned"（`isteamugc.h:380-381`）。
  结构体自己也说了分页怎么读：`m_rgAppIDs[32]` 是固定数组、`m_nNumAppDependencies` 注为
  "number returned in this struct"、`m_nTotalNumAppDependencies` 注为 "total found"
  （`isteamugc.h:604-607`）。所以本仓交出的是一页一个 `GetAppDependenciesResult`
  （`app_ids` 按 Steam 报的数**夹到数组长度**，`dependencies_returned` / `total_dependencies`
  两个数分开保留），并且进了 `CallbackResult`，调用方用 `register_callback` 或回调流就能收全。
  拿 `FnOnce` 接第一页当"完整列表"是不做的东西，写在 `answers.rs` 的模块注释里。
  要不要再包一层"收拢到齐了再回调"？做得到（终止条件就是累加条数 == `total_dependencies`），
  但要动派发：`call_results` 现在是**命中即摘除**（`src/lib.rs:322-323`），多次答案得让条目留在表里，
  而那张表正是 2026-10-13 修过自死锁的地方（回调内注册/丢弃/嵌套派发）；
  另一半代价是语义：真到不齐（Steam 少发一页）时，"等齐再回调"的闭包就**永远不触发**。
  两个代价都摊开了，结论是**先不做**，不改派发、不引入永不触发的形状。

2d 的 5 条变异复算（`E:/tmp/answers_mutants*.sh`，读数一律取自门禁自身而不是管道尾）：
`answered` 的 io 分支改成 `if false` ⇒ M1 判红；`voted_down` 抄 `m_bVotedUp` ⇒ **第一版判绿**，
因为那条测试在第二个用例里只断了 `voted_up` 为真、没断 `voted_down` 为假，补成"每个输出位各自来自
自己的源位"三段断言后 M2 判红；`action_time` 抄 `m_unVersion` ⇒ M3 判红；`was_add_request` 恒 `true`
⇒ M4 判红；删掉 `AddUGCDependencyResult::ID` 那条解码臂 ⇒ M5b 判红。
第一版 M5 想改成"两条臂共用一个 ID"，结果编不过 —— **编不过的变异不算击杀**，只记 UNVIABLE。
另外这轮还抓到自家分类器的错：`grep "^error"` 把失败测试打印的 `error: test failed` 当成"编译失败"，
于是把两条真击杀误报成 UNVIABLE；判据改成只认 `error[E` / `could not compile`。



### 6.1f 两条"测试看不见"的门（2026-10-14）

这两处的共同点不是"重要"，而是**任何测试都不会因为它变红**，所以只能靠门：

1. **Vendored SDK 清单**（`scripts/ci/sdk_vendored.py` + `compare_hashes.py`，基线
   `baselines/sdk_vendored.txt`，46 条 = 45 个 `.h` + `steam_api.json`）。
   绑定是生成后**入库**的，所以有人改了 Valve 头文件时，crate 自己的测试一个都不会红——
   改动只在下一次重生成时显形，而那等于悄悄换了"这个 wrapper 在封谁"。
   判据分三类各自指名（改动/新增/删除语义不同，混成一句"清单不符"就丢了方向）；
   另加两条**工具自检**：清单条目 <10 或 SDK 目录不在 ⇒ `rc=2`（是尺子坏了，不是代码错了）。
   实测控制（每次退出码都从门本身取，不经管道 tail）：追加一行 ⇒ `changed` rc=1；
   伪造一条记录 ⇒ `added` rc=1；清单缺失 ⇒ rc=1 且同时点出"没提交/被 ignore"两种成因；
   改名 ⇒ 被生产者自己的地板抓住（45 < 46，rc=2）；每次还原都用 `cmp` 逐字节确认。
2. **编译覆盖面**（`scripts/ci/compile_coverage.py`）。cargo 对声明了 `required-features`
   的目标是**静默跳过**，`--all-targets` 也不救；所以"`examples` job 绿"从来不等于
   `stream_events` 被编过。门把两头都断言，且都是从 cargo 的 JSON 里取实际编译到的目标名：
   开 `steamworks/futures` ⇒ 必须含 `stream_events`；不开 ⇒ 必须**不含**。
   后半条是对前半条立论前提的自检：哪天 cargo 不再跳过了，这条门要判红并要求重写判据，
   而不是继续"通过"一条已经无意义的断言。本机实测
   `['http','inventory','stream_events']` / `['http','inventory']`。

顺带纠自己一条错账：`.gitattributes` 原写"46 个 Valve SDK 头文件"，`git ls-files --eol`
复算后是 **46 个 vendored 文本文件 = 45 个头 + `steam_api.json`**，已改。
这两条门都按上一条教训补了步骤：**先 `git commit`，再 `git clone` 到干净签出里跑正例**——
上一次就是因为我拿"暂存但未提交"的树去 clone，clone 里根本没有新脚本，跑出来的红是假红。



### 6.1g `CallbackResult` 的流式债（2026-10-14 新发现，已由门钉住）

复算方法级缺口时顺手扫了一遍 `impl_callback!`，量出一个此前没登记的面：
`CallbackResult` 的文档自称"all possible callback results"，而 `CallbackStream` 只认这个 sum type。
按**递归**扫 `src/**/*.rs`（先前用 `src/*.rs` 的宽口径漏了 `src/user_stats/stat_callback.rs` 这种子目录文件）：
本轮动手**前**是 66 个回调定义对 38 个 enum 变体 ⇒ **28 类事件对流式用户不可见**（`register_callback` 路径仍可收）；
动手**后**是 69（新增 3 个聊天回调）对 45 变体 ⇒ 缺 **24** 类。

本轮把与聊天/Timeline/统计答题同族的 7 个接进 stream：
`FriendChatMessageReceived`、`ClanChatMessageReceived`、`UnreadChatMessagesChanged`、
`FriendRichPresenceUpdate`、`TimelineEventRecordingExists`、`TimelineGamePhaseRecordingExists`、
`GlobalStatsReceived`。剩下 **24 个**按模块登记在 `callback.rs` 的 `CALLBACKS_NOT_IN_STREAM`
（ugc 1、inventory 3、parties 3、server_stats 3、html_surface 12、music 2），
不编造理由，只写归属模块。

两道门把它钉成"只能变短"的债：
`every_callback_is_streamable_or_listed_as_debt`（新回调既没变体又没登记 ⇒ 判红）、
`debt_entries_point_at_callbacks_that_are_still_unstreamed`（债目指向的名字若已不存在或已被接进 stream ⇒ 判红），
外加 `the_two_parsers_see_the_whole_crate` 的地板断言（解析器空转就判红，防"违规数 0 = 没在看"）。
变异对照：把 `ClanChatMessageReceived::ID` 那条 `from_raw` 分支删掉 → 交付门判红；
把一个债目改名 → 债目门判红；临时放一个没有任何接线的新 `impl_callback!` → 覆盖门判红。
`CallbackResult` 的文档同时改成"sum type over the callback results the stream can deliver"，
并指向债表 —— 先让声明配得上事实，再用门防止事实倒退。


---

## 0.3 `cargo build` / `clippy` / `test` 实测结果

在 Windows x64 环境实测三条命令：

### `cargo build --all-features` → **通过，零错误**

```
Compiling steamworks v0.13.1
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.42s
```

### `cargo clippy --all-features` → **122 个警告，但需分层看**

| 来源 | 数量 | 性质 |
|------|------|------|
| `steamworks-sys`（bindgen 自动生成的 FFI） | 29 | `transmute(u32→u32)` 冗余 + unsafe 函数缺 `# Safety` 文档节。**自动生成代码，无害**，应通过 bindgen 配置或 crate 级 `#![allow]` 消除噪音 |
| `steamworks`（手写代码） | 约 6 | `redundant field names`（src/lib.rs:354,366、src/server.rs:551,563）、`empty line after doc comment`、`type_complexity` |

**结论**：`cargo build` 干净说明代码能编译，但 `clippy` 的 122 个警告里**绝大多数是 bindgen 噪音**。真正的信号是：这个 crate **没有配置 `-D warnings`**，否则 bindgen 的 29 个警告会直接判红。这印证了「无 CI 门禁」的判断。

值得单独指出：`src/lib.rs:111` 的
```rust
callbacks: Mutex<HashMap<i32, Box<dyn FnMut(*mut c_void) + Send + 'static>>>,
```
被 clippy 判为 `type_complexity`——这个类型正是缺陷 #2（可重入死锁）与 #1（静默覆盖）的载体，重构回调存储结构会一并解决这个警告。

### `cargo test --all-features` → **7 通过 / 11 失败 / 2 忽略**

**11 个失败全部因同一根因**——无 Steam 客户端：

```
thread 'tests::basic_test' panicked at src\lib.rs:780:37:
called `Result::unwrap()` on an `Err` value:
  NoSteamClient("Cannot create IPC pipe to Steam client process. Steam is probably not running.")
```

失败的 11 个：`basic_test`、`user::test_auth_dll`、`user::test_auth_webapi`、`matchmaking::test_lobby`、`matchmaking::test_set_lobby_game_server`、`matchmaking_servers::test_internet_servers`、`networking_sockets::tests::test_socket_connection`、`networking_sockets::tests::test_create_listen_socket_ip`、`networking_types::tests::test_allocate_and_free_message`、`networking_utils::tests::test_get_networking_status`、`remote_storage::test_cloud`。

**通过的 7 个才是关键证据**——它们全是纯逻辑测试，不依赖 Steam 运行时：

```
test networking_types::tests::test_network_identity_ip ... ok
test networking_types::tests::test_network_identity_steam_id ... ok
test networking_types::tests::test_new_ip ... ok
test networking_types::tests::test_set_ipv4 ... ok
test tests::steamid_test ... ok
test user_stats::test_global_stat_cstring_error ... ok
test server::test ... ok
```

这证明了两件事：

1. **不依赖 Steam 的单元测试是可行的**，且现有代码里已存在（SteamId 位运算、IP 解析、CString 错误映射等）。`server::test` 甚至能通过——说明 GameServer 的纯逻辑路径可测。
2. **20 个测试中有 11 个（55%）无法在无 Steam 的 CI 环境运行**，这直接导致 CI 无法有效工作。值得注意的是 `user_stats::test_global_stat_cstring_error` 通过且名字带 `cstring_error`——它验证的正是 CString 错误路径，说明作者意识到了这个风险点，但只在一个地方做了测试覆盖，另外 77 处 panic 点全无测试。

**SuperSteam 据此的决策**：把测试分层为「纯逻辑（CI 必跑）/ Mock FFI（CI 必跑）/ 集成（`#[ignore]`，需真实 Steam）」。目标是把 CI 可运行测试从 7 个扩展到全模块覆盖，而非停在 35% 通过率。

> **实施注记（2026-10-10，分支 `fix/test-tiering` @ `7cd6452`，详见路线图表 #9 行）**：分层已落地（集成档 11 测 `#[ignore]` + CI test job 诚实门禁）。

> **覆盖缺口量化（2026-10-11，八分支集成栈上实测）**：全仓 23 个源码模块、491 个 pub fn（含各模块 `from_raw` 等 trait 实现面），按「函数体含 `sys::` 调用」二分：FFI 绑定面 ≈358（不可离线测，归集成档）；**纯逻辑面 ≈133**，其中已被 40 个离线测试覆盖的集中在 lib/callback/matchmaking/networking_sockets/networking_types/ugc/server 等模块。**剩余未覆盖的纯逻辑面 ≈20 个函数，几乎全为小型访问器与 builder**（matchmaking 的 `LobbyListFilter` 构建器族、networking_types 的消息状态读取器族、friends `get_friend`、ugc 查询结果访问器族、若干 `from_raw`/`raw()` 一行器）。**结论：覆盖扩展的剩余空间是边际的**——没有隐藏着未测的复杂逻辑。**其中有真实契约的子集已落地**（2026-10-11，分支 `test/offline-accessor-coverage` @ `060ad3f`，基点 `6e4a2bc`，未推送，tests-only 3 文件 +164/−0：`AppNetConnectionEnd` 双区间 panic 契约与不相交性、`LobbyKey` 255 字节边界、`StringFilterKind→ELobbyComparison` 六臂全映射、`data_type` 三组抽查、`AppIDs` 三变体访问器、`LobbyListFilter` 构建链回读；门禁同尺 fmt0/build0/clippy 逐项同 122/122/串行 **19-11-2**=7-11-2+12/并行 10×0）；剩余未覆盖仅为 `raw()`/`from_raw` 一行器与纯字段 getter（无契约可锁，声明不做）；其余扩展面只能来自 #7 的 11 个接口缺口（新功能）。⭐**本节指纹的口径边界**：上面「7 通过 / 11 失败 / 2 忽略」只是 **lib 段**——`cargo test` 无 `--no-fail-fast` 在首个失败段后停止，Doc-tests 段（14 例，其中 3 例调 `Client::init()` 必失败）当时从未被执行；这是 lib 段修绿后才现形的掩账，已在 #9 分支一并修掉（3 例改 `no_run`）。Mock FFI 档未做（归覆盖扩展）。

---

## 0.4 `apicall_result.resize` 风险实测与降级

我最初把这项标为「中」风险，理由是「`m_cubParam = usize::MAX` 可 OOM」。**实测后这个描述有两处不准确**，现予修正：

### 修正 1：`m_cubParam` 不是 usize，是 `uint32`

`isteamutils.h:269` 明确定义：
```c
struct SteamAPICallCompleted_t {
    SteamAPICall_t m_hAsyncCall;
    int m_iCallback;
    uint32 m_cubParam;      // <- uint32，不是 size_t/usize
};
```

因此上限是 **4294967295 字节（约 4GB）**，而非 `usize::MAX`。`as usize` 转换在 64 位平台无截断。

### 修正 2：失败行为需分两种情况，且都不能简单称为「OOM」

实测两种路径：

```rust
// 情况 A：超出容量上限（isize::MAX 或布局上限）
v.resize(isize::MAX as usize + 1, 0u8);
// → panic: capacity overflow，退出码 101，catch_unwind 可捕获

// 情况 B：大小合法但物理内存不足
v.resize(2 * 1024 * 1024 * 1024, 0u8);   // 2GB，本机 15.4GB 内存下成功
// → 若内存不足，走 std::alloc::handle_alloc_error → abort()，不可捕获
```

实测数据（本机 15.4GB 内存）：

| 分配大小 | 耗时 | 结果 |
|---------|------|------|
| 512 B | 400ns | 成功 |
| 64 KB | 32µs | 成功 |
| 16 MB | 2.7ms | 成功 |
| 2 GB | 801ms | 成功 |
| 4 GB (u32::MAX) | 875ms | 成功（虚拟内存 overcommit） |

**关键区分**：`resize` 没有 `try_resize` 变体。若值在合法容量范围内但物理内存不足，走 `handle_alloc_error` → **abort 进程，不可捕获**；若值超出容量上限，则是可捕获的 `capacity overflow` panic。

### 风险定级：低，但理由与最初不同

真实触发需要 **Steam 客户端返回畸形的 `m_cubParam`**。触发条件：
- 本地进程间通信（IPC）被篡改
- Steam 客户端自身 bug 或内存损坏

正常运行时 Steam 回调大小通常 < 1KB，最大场景是远程存储文件读取（可能到 MB 级）。攻击者要利用此缺陷需要**已能控制本机 IPC 通道**——此时进程 abort 只是众多后果中最轻的一个。

因此定级为**低**：不是「容易触发」，而是「触发后无优雅降级路径」。

### 仍值得修复的理由

虽然风险低，但修复成本极低且有明确收益：

```rust
const MAX_APICALL_PARAM: u32 = 16 * 1024 * 1024;  // 16MB，远超任何真实回调

let size = apicall.m_cubParam;
if size > MAX_APICALL_PARAM {
    log::error!("apicall param size {} exceeds limit", size);
    continue;   // 跳过而非分配
}
apicall_result.resize(size as usize, 0u8);
```

**顺带修掉一个真实的小缺陷**：`apicall_result` 在循环外声明（lib.rs:163），每次回调都 `resize`。若连续两个回调大小差异大，会反复重新分配。正确做法是放进循环内或复用容量。

### 深挖补充（详见 defect-03-resize-deep-dive.md）

两点主文档此前未覆盖的事实：

1. **u32→i32 静默负数转换**：`GetAPICallResult` 的 `cubCallback` 参数是 `c_int`（`steam_api.h:225`），而 `m_cubParam` 是 `uint32`。`lib.rs:176` 的 `apicall.m_cubParam as _` 在值 > 2^31-1 时**静默变负数**（实测：`0x80000000` → `-2147483648`，`0xffffffff` → `-1`）。且两侧口径不一致——`lib.rs:170` resize 侧用 `as usize` 无截断，C 调用侧变负。这才是 Rust 绑定特有的精确缺陷点。

2. **责任归属修正**：Valve 官方手动派发模板（`steam_api.h:189`）同样是 `malloc(pCallback->m_cubParam)` **无上限**。「缺少上限校验」是 Valve 契约层面的信任假设，非 Rust 绑定引入。对外提 Issue 时只应主张口径不一致这一点，否则会被「官方模板也这么写」合理反驳。

严重度结论不变：**低**。

这是「读了代码以为发现了严重漏洞，实测后降级为低风险但仍有修复价值」的典型案例——我把定级从「中」改为「低」，而不是为了证明最初判断正确而夸大它。

---

## 0.5 `Arc::clone` 性能论据的否证

我最初把缺陷 #8 描述为「Arc 开销大」，并据此把 SuperSteam 的「性能」目标定为「消除每次调用的 Arc 原子操作」。**实测后这个论据不成立。**

### 实测数据（`repro/arc_clone_bench.rs`，1 亿次迭代）

| 模式 | 单次耗时 |
|------|---------|
| `Arc::clone`（现状模式） | **8.026 ns** |
| 借用视图（提案模式） | **0.218 ns** |
| 绝对差值 | 7.809 ns |

### 换算到真实场景

| 场景 | Arc 模式 | 借用模式 | 节省 |
|------|---------|---------|------|
| 60 FPS，10 接口/帧 | 4.816 µs/s | 0.131 µs/s | 4.685 µs/s |
| 144 FPS，10 接口/帧 | 11.558 µs/s | 0.313 µs/s | 11.245 µs/s |
| 240 FPS，10 接口/帧 | 19.263 µs/s | 0.522 µs/s | 18.741 µs/s |

**关键比例**：60 FPS 下每帧取 10 个接口，Arc 克隆总计占帧预算（16.67ms）的 **0.000481%**。

### 测量有效性检验（`repro/arc_validity_check.rs`）

排除「被编译器优化掉」的干扰后：

| 测量方式 | 耗时 |
|---------|------|
| `clone` + `forget`（纯递增，无 drop） | 5.248 ns |
| `clone` + drop（完整生命周期） | 8.970 ns |
| 裸 `fetch_add(Relaxed)` | 6.175 ns |
| `clone` + `strong_count` | 10.975 ns |

`clone+forget`（5.2ns）与裸原子操作（6.2ns）同量级，证明原子递增路径真实存在、未被优化掉；加 drop 后升到 8.9ns，说明 drop 的原子递减贡献约 3.7ns。数据可信。

### 结论：性能论据不成立

绝对差值 7.8ns 真实存在，但**在游戏帧预算语境下完全不可见**——占比 0.0005%，远低于任何可测量阈值。即使按最激进的假设（240 FPS + 每帧 100 次接口调用），也仅占帧预算约 0.12%。

因此：

- **不能说「Arc 开销大」**，这是一个被夸大的性能论断。
- SuperSteam 改用 `Box<Inner>` + 借用视图的**真实理由不是性能**，而是：
  1. **类型安全**：接口对象不能再脱离 `Client` 生命周期存活，编译期阻止 use-after-free
  2. **API 清晰度**：消除 42 处 `self.inner.clone()` 的样板代码，同时消掉 clippy 的 `type_complexity` 警告
  3. **错误处理**：接口获取可以返回 `Result`（指针为 null 时报错），而非 `debug_assert` 后裸指针解引用

我把这一项从「性能缺陷」重新归类为「可选的架构改进」，理由改为类型安全与 API 清晰度。这是「凭代码计数下结论」被实测推翻的第四例——**当时的原话是「`37 处`这个数字本身没错，错的是把它解读为性能瓶颈」。前半句经复查是错的**：`grep -rn "self\.inner\.clone()" src/ | wc -l` 实测 **42 处**（宽口径 `inner.clone()` 为 48 处），且遍历 HEAD~100..HEAD 的历史提交，计数在 33/36/40/42 间跳变，**37 在任何提交上都不可复算**——它要么来自已失效的本地改动，要么是记忆错误。已按可复算口径全文更正为 42 处。性能否证的结论不受影响（8ns/次 × 任何合理次数都远低于帧预算），但「数字本身没错」这句话本身正是清单 A2 要拦的失误。

---

## 0.6 缺陷 #2 的最新事实：部分已修复

在准备提交 Issue 前，我先核查了是否已有重复报告，结果**发现我此前的描述已过时**：

- **Issue #253**（2025-06-29，作者 nigrodev）已精确报告同一缺陷，且根因分析正确
- **PR #259**（james7132，collaborator）**已合并**，通过拆分 Mutex 修复了主要场景
- 维护者 james7132 与 owner Noxime 均已在 #253 表态，倾向方案 4（废弃 `run_callbacks`，只保留 `process_callbacks`）

### 本地代码现状

`src/lib.rs:133-140` 已是拆分后的结构（即 #259 的修复已在代码中）：

```rust
struct Callbacks {
    callbacks: Mutex<HashMap<i32, Box<dyn FnMut(*mut c_void) + Send + 'static>>>,
    call_results: Mutex<HashMap<sys::SteamAPICall_t, Box<dyn FnOnce(*mut c_void, bool) + Send>>>,
}
```

### 实测剩余影响面（`repro/post_fix_deadlock_test.rs`）

| 场景 | 结果 | 状态 |
|------|------|------|
| A：回调内调用 API（`register_call_result`，如 `matchmaking.join_lobby`） | 正常返回 | **已修复**（#259 效果） |
| B：回调内调用 `register_callback` | 死锁 | **仍存在**（需同一把 `callbacks` 锁） |
| C：回调内嵌套调用 `run_callbacks` | 死锁 | **仍存在** |
| D：回调内丢弃 `CallbackHandle`（Drop 持 `callbacks` 锁，src/callback.rs:165） | 死锁 | **仍存在** |

### 补充实测：场景 E（`repro/call_result_chain_deadlock_test.rs`）——四种残余之外的第五种

深挖（见 `defect-02-deadlock-deep-dive.md`）发现上表不完整。`run_callbacks_raw` 的 call_result 分支（`src/lib.rs:180-185`）**在 `call_results` 锁守卫作用域内执行用户闭包**：

| 场景 | 持有的锁 | 需要的锁 | 结果 |
|------|---------|---------|------|
| A（既有） | `callbacks` | `call_results` | 跨锁 → 通（#259 修复） |
| **E（新）** | `call_results` | `call_results` | **同锁自锁 → 死锁（实测）** |

**触发链是现实高频模式**：`request_lobby_list` 结果闭包里 `join_lobby`（服务器浏览器）、`create_item` 结果里 `subscribe_item`（UGC 发布）。`register_call_result` 有 10+ 个公开调用点（matchmaking/remote_storage/ugc）都是潜在触发器。

**关键推论：#253 已讨论的两条方案都不覆盖 E**——
- 方案 4（废弃 `run_callbacks`）：E 在 `run_callbacks_raw` 的**共享分支**，两个入口都走；
- `Arc<dyn Fn>` 锁外调用：只作用于 `callbacks` 表，不触及 `call_results`。

**修法统一**：把守卫作用域缩到 `remove` 为止、闭包移出后锁外调用——一处手法修 A/B/C/D/E 全部五种。仓内已有正确先例：`process_callbacks` 传入的 `callback_handler`（`lib.rs:149`）本就在锁外执行。

### 对缺陷定级的影响

- 缺陷 #2 的**主场景（回调内调 Steam API）已修复**——这正是 #253 报告者遇到的情况，也是实际游戏开发中最常见的形态。
- 但深挖发现**场景 E（call_result 内链式调 API）影响面比 B/C/D 更宽**：`request_lobby_list` → `join_lobby`、`create_item` → `subscribe_item` 都是文档级常见链路，且 E 不受下述任何一条已规划方案保护。
- 维护者已规划的方案 4（废弃 `run_callbacks`）会消除 B 和 C；**E 无效**（在共享的 `run_callbacks_raw` 分支）；D 需另行处理。

**这修正了我此前的表述**：我原先笼统说「回调内注册/派发即挂死」，未区分已修复与未修复部分；后来又写「剩余三种影响面窄」——**第二次修正**：实为四种，且 E 的影响面不窄（见 defect-02 文档）。准确表述是「主场景已由 #259 修复，四种次要场景仍存在，其中 E 为高频链路」。

### 关于提交 Issue 的处置

按「开源 PR 先确认需求存在」的纪律，核查发现**不应新开 Issue**——#253 已存在且维护者已确认。若要贡献，正确做法是在 #253 上补充 B/C/D 三个场景的实测证据（说明 #259 未完全覆盖），而非另开重复 Issue。

**状态更新**：先前会话（bfxh）已在 #253 追评了 A/B/C/D 四场景；**场景 E 已在 #253 追评**——见 [comment-6094802968](https://github.com/Noxime/steamworks-rs/issues/253#issuecomment-6094802968)。

注：Steamworks.NET 覆盖近全量（含上述冷门接口），这是它相对 steamworks-rs 的明确优势，SuperSteam 必须补齐。

### 6.2 GameServer 支持

`server.rs` 已存在 778 行，覆盖 `ISteamGameServer` 主接口 + 5 个子接口（UGC `server.rs:536`/Networking/Utils/NetworkingMessages/NetworkingSockets）；缺 GameServer 侧 HTTP/Inventory/Stats。SuperSteam 提供对称的 Client / GameServer 双模式。

---

## 7. 开发工具与质量保障

### 7.1 CI 门禁（对比 Steamworks.NET 仓库零 CI workflow）

```yaml
# 必须全绿才可合并
- fmt --check
- clippy -- -D warnings
- cargo test                     # 全部 29 模块
- cargo deny check               # 依赖审计
- cargo audit                    # 安全公告
- miri test                      # UB 检测（FFI 用 mock 替代）
```

### 7.2 测试策略

现状问题：20 个测试中 `basic_test`（src/lib.rs:780）依赖真实 Steam 客户端，无 Steam 的 CI 必然 panic，导致该测试实际无法在 CI 运行——14 个模块因此零覆盖。

方案：三层测试
1. **纯逻辑层**（无需 Steam）：SteamId 位运算、错误映射、字符串转换 → 可 100% 覆盖
2. **Mock FFI 层**：用 Rust 模拟 `sys::*` 符号，测试回调分发、队列、生命周期
3. **集成层**：`#[ignore]` 标记，需真实 Steam 才跑

### 7.3 代码生成

现状 build.rs:79 已有 bindgen 基础，但 `.allowlist_var(".*")` / `.allowlist_type(".*")` 带 TODO 未裁剪，生成 27k 行/平台的冗余绑定。

方案：基于 `steam_api.json`（SDK 自带接口清单）精确生成，仅导出真实接口，减少编译体积与符号冲突面。

---

## 8. 迁移路径

### 从 steamworks-rs 迁移

```rust
// 现状：需手动管 Arc、回调静默覆盖风险、错误 panic
let client = Client::init()?;
let _cb = client.register_callback(|p: PersonaStateChange| { ... });  // 二次注册会覆盖

// SuperSteam：类型安全、零 panic、async
let client = Client::new()?;
client.on_persona_change(|p| { ... });        // 支持多监听者
let friends = client.friends().get_friends().await?;
```

### 从 Steamworks.NET 迁移

```csharp
// 现状：无 async、每次 SetStat 触发 AllocHGlobal、错误靠 bool
SteamManager.Init(...);
SteamUserStats.SetStat("kills", 100);   // 返回 bool，慢

// SuperSteam
let client = Client::new().await?;
client.user_stats().set_stat("kills", 100).await?;   // Result + 零分配 + async
```

---

## 9. 路线图（按依赖排序）

### 阶段 0：修复面实测统计

动手前先量清每项的实际工作量，避免凭「N 处」这类数字估工期：

| 缺陷 | 需改位置 | 改动性质 | 风险 |
|------|---------|---------|------|
| #2 死锁残余 | 3 类操作（`register_callback`/嵌套派发/`CallbackHandle::Drop`） | 改锁策略，涉及公开 API 行为 | **高**（需与维护者确认方向） |
| #1 静默覆盖 | `callbacks` 值类型 `Box` → `Vec<Arc>` | 内部结构，公开签名不变 | 中 |
| #5 CString panic | **77 处**，跨 15 文件 | 每处加错误分支，机械但量大 | 低 |
| #6 UGC builder | `ugc.rs` 中 **22 个含 panic 点的 builder 方法**（设计期曾按全部 39 个 `-> Self` 方法估面；普查后 17 个 bool/u32 方法仅 `debug_assert!`、无 panic，不需转换，见 defect-06 A2） | 22 个改 `Result<Self, E>`，**破坏性 API 变更** | **高**（需 major 版本） |
| #4 空指针 | **23 个公开方法**（lib.rs 17 + server.rs 6） | `debug_assert!` → `disabled` 标记（timeline 式，零破坏） | 低（见 0.7 节方案对比） |
| #3 resize | 1 处（lib.rs:170） | 加上限 + 移入循环内 | 低 |

**两个关键判断**：

1. **#6 是唯一需要破坏性 API 变更的项**。22 个 builder 方法从 `-> Self` 改为 `-> Result<Self, E>` 会打断所有现有调用方，因此不适合作为上游 PR 的独立提交（需 major 版本）——但在 SuperSteam 这种新 crate 里可以直接这样设计。
2. **#5 工作量此前被低估**。77 处跨 15 个文件，虽然单处改动简单，但需为每处设计合适的错误类型与传播路径，不是纯机械替换。

这 7 项难度差异极大（从 #3 的 1 处到 #6 的 22 处破坏性签名），笼统按「1 个月」估工期不现实。按依赖与风险分批更合理：#3/#5 可立即做，#4/#1 需 API 讨论，#6 需版本规划，#2 需等上游方向。

### 阶段 1：正确性修复（最高优先，1 个月）
独立成项目，因为已实证的缺陷本身就是高价值交付物。每项均有 `repro/` 下的运行时证据：

| 修复项 | 对应缺陷 | 证据脚本 | 状态 |
|--------|---------|---------|------|
| 死锁：call_result 处理器内链式调 API（场景 E，守卫作用域收缩） | #2 | `call_result_chain_deadlock_test.rs` + `..._fixed.rs`（A/B） | **已实施**（`src/lib.rs:180-194`，+8 行，零签名破坏；build/test/clippy/fmt 四门禁同尺复测无回归；已拆独立分支 `fix/call-result-handler-self-deadlock` commit `20b800a`，只含本修复） |
| 死锁残余场景修复（回调内 `register_callback` / 嵌套派发 / 丢弃 `CallbackHandle`，场景 B/C/D） | #2 | `post_fix_deadlock_test.rs` + 在树 `callback_multi_registration_tests` 四个重入测试 | ✅ 2026-10-10：分支 `fix/callback-dispatch-reentrancy` @ `ebd1183`（**基于 #1 分支尖端 `5d2bcc9` 的依赖分支**，未推送——put-back 设计建立在 #1 的 `(seq, Box)` 表上，上游按后续 PR 排序）；派发改为 **put-back + 墓碑**：整桶取出→放锁调用→按 seq 归并回插，`pending_removal` 墓碑集让「回调内丢自己句柄/回调内替换注册」在在途期间依然生效（⭐在树测试抓到「桶被重入注册重建、条目仍在途」的墓碑盲区——判据必须按「seq 表中缺失」而非「桶缺失」）；零公开 API 变更 ⇒ 不再依赖 #253 方向拍板（上游若偏好 `Arc<dyn Fn>` 属实现替换、语义等价）；回调 panic 从毒化锁改为丢弃在途条目（行为变更已记档）；3 文件 +192/−7；门禁同尺（对 base `5d2bcc9`）：fmt 0 / build 0 / clippy kind 多重集逐项同（122/122）/ 串行 **16-11-2** = 12-11-2 + 4 新测（超时包装防挂死：回归变成断言失败而非吊死套件）/ 并行 10×0 异常 |
| 同类型多回调注册支持（Vec 值，消除静默覆盖） | #1 | `callback_multi_registration_tests`（lib.rs 内 5 测，离线） | ✅ 2026-10-10：分支 `fix/callback-multi-registration` @ `5d2bcc9`（基点 `6e4a2bc`，未推送）；表改 `Vec<(seq, Box)>` + Handle 持 seq 修陈旧误杀 + `register_replacing_callback` 保 3 处文档化替换语义（5 个 mem::forget 站点的句柄泄漏同时消除；sockets get_or_create 走 Weak 缓存零改动）；6 文件 +277/−42；门禁同尺：fmt 0 / build 0 / clippy kind-diff 0/0 / 串行 12-11-2 = 基线 7-11-2 + 5 新测 / 并行 10×0 segv；详见 defect-01 实施记录 |
| CString panic 零破坏降级（58 处；19 处 builder 归 #6） | #5 | `cstring_nul_degrade_tests`（lib.rs + remote_storage.rs 共 12 测，离线） | ✅ 2026-10-10：分支 `fix/cstring-nul-panic` @ `5bf2336`（基点 `6e4a2bc`，未推送；⭐原 `e7f3775` 因脚本把 `"\x00"` 写成**原始 NUL 字节**→git 判二进制→EOL 清洗停用→python CRLF 入库，diffstat 曾虚胖为 +1637/−1283，规范化+转义后 amend 定稿，**真 diffstat 16 文件 +424/−70**，指纹 19-11-2 与修前逐字同）；`pub(crate) cstring()` helper + 按返回通道降级（bool→false / Option→None / Result→既有 Err / 句柄→0 / 结构体名→空 / 回调→cb(Err(IOFailure))）；门禁同尺：fmt 0 / build 0 / clippy kind-diff 0/0 / 串行 test 19-11-2 同（11 失败全为本机 NoSteamClient 环境性）/ 并行 10×0 segv；详见 defect-05 实施记录 |
| UGC builder 全链路 Result 化（22 方法：13 UpdateHandle + 9 QueryHandle） | #6 | `builder_result_tests`（ugc.rs 内 8 测，离线） | ✅ 2026-10-10：分支 `fix/ugc-builder-result` @ `f54fb6c`（基点 `6e4a2bc`，未推送）；22 个 `-> Self` 改 `-> Result<Self, SteamError>`（NUL→InvalidParameter / canonicalize NotFound→FileNotFound 其余 io→IOFailure / sys Set* 假→InvalidParameter），`SteamParamStringArray::new` → `Option`（失败回收已转换串）+ `init_for_game_server` 保 `-> bool` 骑缝（NUL→false），`#[must_use]` 从 6 个被转方法摘除（Result 隐含）；行为改善：QueryHandle 串方法 release 下 debug_assert 静默失败改回 Err；4 文件 +466/−213；门禁同尺：fmt 0 / build 0 / clippy kind-diff 0 新增 1 消失（旧 `into_iter` lint 打在被重写的 `SteamParamStringArray::new` 上）/ 串行 test 15-11-2 = 基线 7-11-2 + 8 新测（11 失败名单逐字同，全为本机 NoSteamClient 环境性）/ 并行 10×0 segv；离线测试 helper 用 null `ISteamUGC` + `handle: None`（QueryHandle::drop 会拿 null ugc 调 ReleaseQueryUGCRequest，Some 即段错误——第一轮门禁实测抓到）；详见 defect-06 实施记录 |
| release 模式空指针守卫（`unavailable` 标记，timeline 式零破坏） | #4 | `unavailable_guard_tests`（lib.rs 内 3 测，离线） | ✅ 2026-10-10：分支 `fix/null-guard-unavailable` @ `6abc953`（基点 `6e4a2bc`，未推送；两提交：`15c0108` 守卫本体 + `6abc953` 修测试间歇崩溃）；23 结构体 `unavailable` 字段 + 258 守卫，20 文件 +1041/−0；门禁 build 清 / test 10 通过 11 失败 2 忽略（HEAD 同尺 7/11/2，+3 全为新测）/ clippy 留档逐行同 / fmt 清；详见 defect-04 实施记录 |
| apicall 大小上限 + `apicall_result` 移入循环内 | #3 | `resize_oom_test.rs` + 在树无 Steam 单测 `call_result_size_bound` | **已实施**（分支 `fix/call-result-size-cap` commit `ca23aab`：`MAX_CALL_RESULT_SIZE=64MiB` + `call_result_size_ok` 兼管 `i32` 口径 + 缓冲移入循环；test 8 通过/11 失败/2 忽略、clippy 121、fmt，均与基线同） |
| 无 Steam 环境可跑的单元测试 + CI 门禁 | #9 | `cargo test` 实测（0.3 节） | ✅ 2026-10-10：分支 `fix/test-tiering` @ `7cd6452`（基点 `6e4a2bc`，未推送）；11 个 Steam 依赖测试标 `#[ignore = "requires a running Steam client"]`（`--ignored` 仍可跑）+ 3 个文档示例改 `no_run`/`rust,no_run`（编译检查保留）；`ci.yml` 新增 `test` job（ubuntu+stable：`cargo test` pipefail+tee，门禁断言每个 result 段 `ok` 且 >0 passed 0 failed、ignored 数回显日志——**只跳不跑不算绿**）；⭐**实测抓到 cargo fail-fast 掩账**：`cargo test` 无 `--no-fail-fast` 在首个失败段后停止 ⇒ 此前一切指纹（7-11-2 系）只含 lib 段、Doc-tests 段从未执行过，lib 修绿后 3 个文档示例失败才现形（它们调 `Client::init()`）——这 3 处 `no_run` 因此属于本修复而非扩权；离线指纹：lib 7-0-13 + Doc-tests 14-0-0、exit 0，并行 10×0 异常，clippy 多重集与 base 逐项同（122/122），fmt 清；Mock FFI 层（设计决策三档的中档）未做，归覆盖扩展 |

注：`Arc → Box` **不属于阶段 1**——它不是正确性问题，且实测无性能收益（见 0.5 节）。

> **状态对账（2026-10-13）**：上表各行写的是 2026-10-10/11 当时的分支本地状态，其中「未推送」已全部失效——九支已于 2026-10-11 经授权推到 fork `bfxh/steamworks-rs` 并对上游开出 **#350-#358**（映射：#350=#2E `20b800a`、#351=#3 `ca23aab`、#352=#1 `5d2bcc9`、#353=#4 `6abc953`、#354=#5 `5bf2336`、#355=#9 `7cd6452`、#356=#6 `f54fb6c`、#357=#2重入 `ebd1183`、#358=覆盖 `060ad3f`；本轮已逐支核过 `本地 head == fork/<branch> head`）。各行保留原文不改，是因为它们记录的**门禁尺与指纹**仍是那些分支的依据。CI 现状与本地等价测量见 `ci-local-evidence-2026-10-13.md`。

### 八分支集成栈验证（2026-10-11，分支 `integration/stack8` @ `e3730c5`，仅本地）

推送拍板的前置验证：八个修复分支都基于同一 master `6e4a2bc`（#2 残余重入分支基于 #1 尖端，为栈中唯一依赖分支），合并冲突面实测。按 **#2E → #3 → #1 → #4 → #5 → #6 → #9 → #2重入** 顺序合并，结果：

- **干净自动并入**：#2E、#3、#1、#9、**#2重入**（重入分支对栈零冲突，仅其不知道的 #5 测试助手初始化器需补 `pending_removal` 字段，amend 进合并提交）。
- **冲突地图**（18 文件 39 块，全部已解）：`networking_messages.rs`×1（#4 守卫 × #1 替换语义 → **守卫 + `register_replacing_callback` 合取**，#4 侧的 `mem::forget(register_callback)` 是它不知道 #1 机制时的替代品，弃用）；#5 合并 22 块（app/friends/input/matchmaking/user_stats 的守卫×cstring → **#4 守卫 + #5 降级合取**；utils 的回调站 → **#5 转换 + #1 替换回调**；lib.rs 助手锚点 → 双留）；#6 合并 15 块（13 个 builder → **#6 Result 体 + #4 守卫，守卫在 Result 世界返回 `Ok(self)` 保持 no-op 语义**；ifgs → 守卫+cstring 形态；`SteamParamStringArray::new` → **#6 的 Option 版**，否则其唯一调用方 tags 编译不过）；跨分支结构体演化（#4 `unavailable` + #1 `next_seq`/`replacing`）⇒ #5/#6 的测试构造器补字段（fixup `74328bb`）+ lib.rs 双助手拼接补一个失闭合括号。
- **合并前修掉的分支内缺陷**：#5 的 `lib.rs`/`remote_storage.rs` 被脚本写入**原始 NUL 字节**（`"\x00"` 转义丢失）→ git 判文件为二进制（`i/-text`）→ EOL 清洗对它停用 → python 的 CRLF 输出原样入库 ⇒ 分支 diff 虚胖（真变更 +424/−70，账面曾是 +1637/−1283）⇒ 规范化后 amend 定稿 `5bf2336`。**六分支其余全部审计干净**。
- **集成指纹**：fmt 0 / build 0 / clippy kind 多重集对 base **0 新增 1 消失**（`into_iter` lint，打在 #6 重写的 `SteamParamStringArray::new` 上）/ 串行 lib **40 通过 0 失败 13 忽略** + Doc-tests 14-0-0、exit 0（40 = base 7 + #3 1 + #4 3 + #5 12 + #1 5 + #6 8 + #2重入 4，逐支计数对账闭合）/ 并行 10×0 异常（含重入分支后复测）。
- **结论**：栈是可合并的；推送时按 PR 逐支顺序进上游，冲突解法以本节为参照。若上游 master 已前进，需在最新基点上重放本节验证。
- **A5 上游查重（2026-10-11 首跑）**：被环境阻塞——gh GraphQL/REST、WebFetch、`git ls-remote` 全通道对 github.com 持续 TLS 握手超时（6 次，含退避与 >90s 挂起）。**任何上游 PR 提交前必须重跑 A5**。
  - **A5 重跑（2026-10-13，网络已恢复，PR 已提交后补做）**：
    - 基点时效：`git fetch origin` 后 `origin/master` **仍是 `6e4a2bc`**（`6e4a2bc..origin/master` 计数 0），即本节集成验证仍站在最新基点上，无需重放。
    - 重复报告检索：对 deadlock / call_results lock / multiple callbacks / CString / unwrap panic / resize / null / UGC builder Result / test ignore CI / unavailable 十组关键词跑 `gh search prs --state open`，命中项除本仓九支（#350-#358）外只有 #342（NetworkingIdentity）、#347（glyph 段错误，仅 `input.rs`）、#245（收紧 unsafe）。**九项缺陷的重复报告 = 0 条**，A5 通过。
    - ⚠️ 前瞻风险（不是重复，是冲突面）：#245（作者 james7132）改动 22 个源文件，与本仓九支高度同域（`lib.rs`/`app.rs`/`friends.rs`/`input.rs`/`matchmaking.rs`/`remote_storage.rs`/`ugc.rs`/`server.rs`/`networking_*` 等）。若上游先合 #245，九支需按其节记录的冲突地图在新基点重放一遍；本节的 52-0-13 指纹届时作废、须重测。
- **上游 PR 草案**：`upstream-pr-drafts.md`（九支的标题/body 要点/拍板清单/推送顺序 + A5 结果，2026-10-11 定稿）。
- **Mimosa 深扫（2026-10-11，推送前尽调）**：扫集成树 `wt-integration`（九支改动并集）——**findings 0**、依赖受影响包 0、封印 `sha256:8aadaadc2842802c90c68f728f01a4f191b48cb956456b0cfb773dba124593bb`。证据边界：静态分析、调用图部分不完整（动态派发超规模）、verdictEffect `none`，**非运行时验证、不据此宣称"安全"**；1 条离线依赖 advisory 匹配为 context-only（包名未入封存物，疑似可选依赖 `image` 的历史 RUSTSEC，属上游依赖面、非本仓引入，推送不受阻）。
- **分支内账目修正记录**：#5 真实 diffstat +424/−70（原报 +1637/−1283 含行尾/NUL 污染噪声，已 amend 定稿 `5bf2336`）；#4 尖端为 `6abc953`（含测试间歇崩溃修复的第二提交），20 文件 +1041/−0。
- **集成栈复测（2026-10-13，`integration/stack8` @ `c8b80c9`）**：本节上条指纹是在 `e3730c5`（stack7 + #358 并入前的最后合并点）量的；`c8b80c9` 又并了 `test/offline-accessor-coverage`（#358 / `060ad3f`）。同尺复测（`-j 1`、`CARGO_INCREMENTAL=0`、`RUSTFLAGS="-C debuginfo=0"`、独立 target）：**build 0** / 串行 lib **52 通过 0 失败 13 忽略** + Doc-tests **14-0-0**、exit 0。账目闭合：52 = 上条的 40 + 12，而 #358 的 `git show 060ad3f | grep -c "^+ *#\[test\]"` = **12**（其 `+...fn ` 计数 13 里含 1 个非 `#[test]` 助手，不作断言依据）。忽略数与 doc 段不变，符合 #358 只增离线测的预期。工作树干净（`git status --porcelain` 0 文件）。

关于上游协作的处置，见第 11 节末尾（先检索重复报告，且需维护者确认方向）。

### #253 方向复核与 D-only 窄版（2026-10-13，分支 `fix/callback-handle-drop-deadlock` @ `c6ca1fd`，仅本地）

复核 `#253` 全量评论后的事实：`james7132` 2025-06-25 列 4 个方案并表明偏好 **option 4**
（废弃 `run_callbacks`、以 `process_callbacks` 为唯一入口），维护者 `Noxime` 当场回
「I agree, I've been thinking of option 4」；2025-06-29 `#259` 只解决了 A 类。此后无人推进 option 4。

**与 #357 的口径矛盾（此前从未向维护者交代，两 PR 均 0 评论）**：#357 body 只写「B/C/D 三案」，
未提 option 4。按 option 4 的设计读法：**B**（回调内 `register_callback`）随注册 API 消失、
**C**（嵌套 `run_callbacks`）随函数消失，**只有 D 活下来**——`CallbackHandle::Drop` 拿 `callbacks`
锁（实测 `src/callback.rs:165`）与用哪个派发器无关。对照之下 **#350（场景 E）不在退役堆里**，
option 4 下 `call_results` 自重入依然存在。

**「代码级 D-only 窄版」不存在**（这是做窄版前先量出来的）：`ebd1183` 是一个不可拆机制
（dispatch 整桶取出→放锁调用→按 seq 归并回插，`pending_removal` 墓碑专司 D），B/C 只是「放锁调用」
的副产物；`ebd1183` 新增 4 测中**仅 1 测真正测 D**。唯一能独立于 B/C 的 D 修法是
`Box<dyn FnMut>` → `Arc<dyn Fn>`（实测表定义在 `src/lib.rs:112`、可变调用站点 `iter_mut()` 仅 1 处），
但那是**破坏性公开签名变更**（现收 `FnMut`，换 `Fn` 会让「只改捕获环境」的闭包不再满足约束），属 major
范畴，且我在 #253 里自己写过「未在本仓原型验证」。⇒ 按用户拍板走**缩声称 + 缩测试**，机制不动。

**窄版做法与实测**：`git cherry-pick --no-commit ebd1183` 后删掉 B/C/替换语义三测与只被 C 测引用的
`NestedTestCallback`（`dispatch_with_deadlock_guard` 为 4 测共用，保留）。
- 机制零改动的证据：与 `ebd1183` 的 `git diff` 只落在 `src/lib.rs` 且**三个差异块全在
  `mod callback_multi_registration_tests` 内**；`src/callback.rs`/`src/server.rs` 与其**逐字一致**。
- 同尺门禁（`-j 1`、独立 target）：fmt --check **0** / build --lib **0** / clippy **0**（121 条，
  与 base `5d2bcc9` 的 121 条**逐类差异 NONE**）/ `cargo test` **0**：lib **24 通过 0 失败 2 忽略** + doc 14-0-0。
- 断言「窄版不宣称 B/C」的实测支撑：D 测 `dropping_own_handle_inside_callback_unregisters_it ... ok` 在结果里；
  `registering_from_inside|nested_dispatch_inside|replacing_registration_from_inside` 三个名字**出现 0 次**（是被删、不是被跳）。
- ⚠️ **24-0-2 是「本机 Steam 在跑」态**（与历史 12-11-2 的无 Steam 态不同尺，不可直接比）；
  同环境 base 对照已出：`5d2bcc9` 同尺 `cargo test` **exit 0，lib 23-0-2 + doc 14-0-0** ⇒
  **24 = 23 + 1**，差值恰为新增的那 1 个 D 测，账闭合。

**对外三连已执行（2026-10-13，用户授权）**：
1. 推分支到 fork：`git push` rc=0 且 **`ls-remote` 比过 SHA**（local=remote=`c6ca1fdd…`）才判成功，不靠 push 输出。
2. 开 PR **#359**（`gh pr list` API 反查到 `head=fix/callback-handle-drop-deadlock` 才算成立）：
   base=master（沿用 #357 先例），body 里明写「增量应对照 #352 尖端 `5d2bcc9` 看，别按 master 的
   +363/−43 账面误读」；账面 `changedFiles=6 +363/-43`、`mergeable=MERGEABLE`；对照 #357 同尺 `+463/-43`
   ⇒ 差额 100 行正好等于删掉的三个 B/C/替换测试（`git diff` 报的 100 deletions）。
3. 在 #357 发口径交代评论：`issuecomment-6130016804`（API 核验 id/作者/长度 1459）。
   评论里的 `src/callback.rs:165` **先按上游 master `6e4a2bc` 复核过行号**（Drop 的 `callbacks.lock()`
   实测在 165；#352 之后才是 169），并在正文声明了行号基准。
- #359 的 CI 与其它九支同态：`completed/action_required`（fork 首次贡献者闸门），仍需维护者批准。

### 已公开数字的回算审计（2026-10-13，A2 尺）

把十支 PR body 里每个数字用可复算的尺重测（diffstat 一律走 `git diff --numstat base..tip`，
结构计数走 diff 自身的增删行并与正则口径交叉验证）：

- **复现成立**：#350 +10/−2；#352 6 files +277/−42；#353 20 files +1041/−0 与 23 结构体；
  #354 16 files +424/−70 与「58 处」（diff 删掉 58 行 `CString::new`：55 同行 `unwrap` + 2 同行
  `expect` + 1 处跨行）；#356 4 files +466/−213 与「13 UpdateHandle + 9 QueryHandle」（按 impl 块分别
  数出 13/9）；#357 3 files +192/−7；#359 3 files +92/−7（= #357 减掉 100 行被删测试，账闭合）。
- **复算不出并已公开更正**：**#353 的「all 258 guarded methods」——真值 246**。
  尺：`git diff 6e4a2bc..6abc953` 新增行里 `if self.unavailable` **246** 条；穷举 `unavailable`
  相关新增行的形态，守卫**只有这一种写法**（另 8 行是 `self.unavailable` 的非守卫引用），
  246 补不到 258 ⇒ 258 是早期工作计数带进来的、从未被复算。
  更正评论 `#353 issuecomment-6130140993`（API 核验 id/作者/长度 939）；**body 故意不改**，按 D1
  保留原文让读者看见错在哪、用什么尺复算。
- **挂在基线上的数字（第二批回算，全部成立）**：
  - #355「**11 of the 20** 单测无 Steam 即失败」：`git grep -c "#\[test\]" 6e4a2bc` 求和 = **20**；
    master 上原有 `#[ignore]` **2** 处（都在 `user_stats.rs`，非本次所加）；`7cd6452` 新增
    `#[ignore = "requires a running Steam client"]` **11** 处 ⇒ **20 = 7 可跑 + 11 依赖 Steam + 2 原有忽略**，
    与 7-11-2 指纹自洽。
  - #358「**491** pub fn」：两把互不相干的尺都得 491（`git grep -h "pub fn" | wc -l`；以及对
    `pub [unsafe] fn` 独立正则并剔掉 17 处 `pub(crate)`）。
  - #358「~**358** FFI / ~**133** 纯逻辑」：用**大括号配对**取真实函数体（先前"切到下一个 pub fn"的
    笨法会把私有helper 吞进来、虚增 FFI 数），按体内是否出现 `sys::` 分类 ⇒ 实测 **359 / 132**，
    与声称 ±1；body 用的是 `~`，且「不直触 sys::」≠「纯逻辑」（有的走 helper），故此类划分本就依赖尺子口径。
  - 顺带交叉验证：#359 声称的「lib 24 = base 23 + 1」与基线普查一致
    （master 20 个单测 + #352 加 3 = 23，窄版再加 1 = 24）。
- **互指与依赖顺序核查（2026-10-13，全部成立）**：十支 base 全是 `master`，依赖只写在正文、不由 git
  结构保证。关键顺序 #352 必须先于 #357/#359 落地——两支 body 都写明了（#359 还引了 option 4 与
  #357 的宽/窄配对）。两处**不影响正确性的澄清缺口**（仅记档、不改）：#354 用分支名（ugc-builder-result）
  而非编号指 companion，#356 未回指 #354。
  - 顺带把一对可能撞车的数对上：#354「UGC builder 里 19 处 panic 点故意不在此转」与 #356 自述的
    「CString 11 + canonicalize 2 + assert 13 + tags expect 1」是**两个不同范围**，且各自成立——
    `git diff 5bf2336..f54fb6c -- src/ugc.rs | grep -c "^-.*CString::new"` = **19**（#354 的量）；
    #356 的 11 是 22 个被转方法内的子集，2+13+1=16 是另外三类，合计 11+16=**27** 为 #356 转掉的 panic 点。

- **审计中我自己的两把窄尺（记档防复用）**：单行正则数「58」只数到 54（rustfmt 把一处 `.unwrap()`
  折到下一行）；同一类正则又漏了跨行泛型签名 `pub fn tags<S: AsRef<str>>(`，一度误判该方法不存在
  （实测在 `src/ugc.rs:1028`，返回 `Result<Self, SteamError>`）。⇒ **结构计数一律以 diff 增删行为主尺，
  正则只作交叉验证**。

### 游离 WIP 落盘（2026-10-13，分支 `wip/callback-stream` @ `4e9b48e`，仅本地）

主工作树 `steamworks-rs/`（`6e4a2bc master`）上压着上一会话留下的 5 个未提交件
（`Cargo.toml +6` 的 `futures` feature、`src/lib.rs +14/−2`、`src/callback_stream.rs` 84 行、
`examples/stream_events.rs` 40 行、`count_cstring_panic.py` 65 行），**此前不在任何分支上 = 唯一副本**。
已按「备份 → `git stash create` 无损捕获（`8fc6ffe`，实测不碰工作树）→ 独立 worktree 落盘」收进分支，
5/5 sha256 与原字节一致（git 侧仅 CRLF→LF 归一，原 CRLF 件在 `_wip-backup-2026-10-13/raw/` 保真）；主工作树未被切换、状态原样。详见 `_wip-backup-2026-10-13/MANIFEST.md`。

两条要记住的结论：
1. **重叠**：`lib.rs` 的 8 行守卫改造与 `fix/call-result-handler-self-deadlock`（`20b800a` / PR #350）**逐字相同**，−2 行全属该项；此分支独立的只有 futures 那条线。上游收下 #350 后，这几行 rebase 即冗余，别再提第二份。
2. **实测这条 WIP 是坏的（有对照组）**：默认特性下 `cargo build --examples` 与 `cargo test` 均 exit **101**（`E0432: unresolved import steamworks::CallbackStream`——`stream_events.rs:19` 无条件引用只在 `--features futures` 下导出的符号，而该 example 没声明 `required-features`）；对照 `fix/test-tiering @ 7cd6452` 同尺 exit **0**。加 `--features futures` 则 build exit 0。修法一行（`[[example]] required-features = ["futures"]`），**未执行**——WIP 保持原样待拍板。
   - **已修（同日，用户拍板后）**：提交 `e5b861b`（分支变 `6e4a2bc → 4e9b48e → e5b861b`；`4e9b48e` 仍保持逐字原样，修复单独成 commit）。修后同尺复测：fmt --check **0**、`build --lib` **0**、`build --examples` **0**、`build --features futures` **0**、`build --features futures --examples` **0**、clippy **0**（121 warnings，与默认态逐类同）、`cargo test` **0**（lib **18-0-2** + doc **14-0-0**）。`cargo fmt` 只动了两处排序（示例的 `use` 行、lib.rs 的 `mod app;`），无逻辑改动。
   - **18-0-2 与历史 7-11-2 指纹的差异已归因，不是本次改动的性质**：本机此刻 **Steam 客户端在跑**（`tasklist` 实测 `steam.exe` PID 24100 + `Steam++.exe`/`steamwebhelper.exe`），那 11 个 Steam 依赖测因此**真跑并通过**而非 `NoSteamClient` 失败（整树耗时 56s vs 离线 2.4s 也印证）。
   - **一处一次性抖动 + 两处我自己的错，记档防误读**：(a) `--features futures` 首轮报 `networking_sockets::tests::test_socket_connection` 1 失败（17-1-2），但该测**隔离跑 3/3 过（带/不带 futures 各 3 轮全过）**、整树复跑 **3/3 全过（18-0-2）** ⇒ 判为联机测的一次性抖动，非 futures 特性所致，也非本次修复引入。(b) 我先前两次把 `#355 --ignored` 对照测出 exit 101，**是我命令行写错**（`--ignored` 要放 `--` 之后、`-j` 不是 libtest 选项，报的是 `error: Unrecognized option: 'j'`），不是测试失败——已作废该结论。(c) 想用 `--test-threads=1` 串行复现"场景 E 活体卡死"，但日志停在第 32 行、进程仍在，**stdout 缓冲使"卡死"与"慢"无法区分 ⇒ 该观测不可判读、已收掉，不据此下任何结论**。

### 阶段 2：架构重构（2-3 个月）
- [ ] Arc → Box + 借用视图（理由：类型安全 + API 清晰度，非性能）
- [ ] FFI crate 隔离 + `#![forbid(unsafe_code)]`
- [ ] 异步适配层（Future + Waker）

### 阶段 3：覆盖补齐（2 个月）
- [ ] 必补缺口：Inventory（内购）、HTTP（自建后端）、ParentalSettings（合规）
- [ ] GameServer 全套对称支持（服务器侧场景）
- [ ] 可选：HTMLSurface、Parties 等按需求决定

### 阶段 4：生态（持续）
- [ ] 交互式文档 + 每模块示例
- [ ] 性能基准套件（对比三者）

---

## 10. 验证状态总表

| # | 缺陷 | 验证方式 | 状态 |
|---|------|---------|------|
| 1 | 同类型回调静默覆盖 | 代码检视（`HashMap::insert` 语义）+ Rust 模拟复现 | **已证实** |
| 2 | 可重入死锁 | `post_fix_deadlock_test.rs`（A/B/C/D）+ `call_result_chain_deadlock_test.rs`（E）：主场景已修复，B/C/D/E 四种仍死锁 | **已复现（E 使影响面回升）** |
| 3 | `resize` 无上限 | `resize_oom_test.rs` 实测 4 种尺寸 + 类型语义分析 | **已实测，风险降级为低** |
| 5 | 77 处 CString panic | `cstring_panic_test.rs` 实测 NUL 输入触发 panic + 调用点分类 | **已复现** |
| 7 | 接口覆盖缺口 | 脚本比对 47 个头文件 vs lib.rs + 逐接口功能分类 | **已核实，口径已修正** |
| 9 | 测试可用性 | `cargo build/clippy/test` 三条命令实测 | **已实测（7/11/2）** |
| 4 | release 裸指针解引用 | `null_guard_test.rs`：debug 下 panic(101) vs release 下 Illegal instruction(132) | **已复现** |
| 6 | UGC builder panic | `ugc_builder_panic_test.rs`：25 个 panic 点复现；归因订正为暂存语义（defect-06） | **已复现（归因已订正）** |
| 8 | 42 处 Arc 克隆开销 | `arc_clone_bench.rs`（1 亿次）+ `arc_validity_check.rs`（排除优化干扰） | **已实测，性能论据否证** |
| 10 | Steamworks.NET `AllocHGlobal` 慢 | 公开 Issue #106 + 源码检视 | 已分析（对比项，非本仓缺陷） |

### 验证过程中的五处自我更正

1. **`SteamAPI_ManualDispatch_Init` 不存在** —— 错。头文件定义了，绑定生成了，调用正确。（见 0 节）
2. **`callback` 变量数据竞争** —— 错。它是栈上局部变量，各线程独立栈帧，不存在跨线程共享。真实缺陷是可重入死锁。（见 0 节）
3. **`m_cubParam = usize::MAX` 可 OOM** —— 错。它是 `uint32`，上限 4GB；且失败行为分 `capacity overflow` panic 与 `handle_alloc_error` abort 两种。风险定级由「中」降为「低」。（见 0.4 节）
4. **42 处 Arc 克隆构成性能瓶颈** —— 错。实测 8ns/次，占帧预算 0.0005%。`Box<Inner>` 改用理由从「性能」改为「类型安全与 API 清晰度」。（见 0.5 节）
5. **缺陷 #2 描述为未修复的死锁** —— 不完整。Issue #253 已报告、PR #259 已合并并修复主场景。我未核查上游状态就下了「仍存在」的结论。（见 0.6 节）
6. **「61 处 `debug_assert!(!x.is_null())`」** —— 数字错。61 是全部 `debug_assert` 的总数（含 `debug_assert!(ok)` 这类返回值检查），真正的 null 指针检查只有 **23 处**（lib.rs 17 + server.rs 6）。（见 0.7 节）

这五处更正的共同点：**都是「基于局部信息得出结论」与「核查完整事实」之间的偏差**。前四次偏差在严重度（把「看起来危险」当成「实际有害」），第五次偏差在时效（未核查上游就已修复）。其中四处如果不去实测或核查，会一直以「专业分析」的姿态留在文档里误导后续决策。

### 验证状态全表（10 项，无未复现项）

原先标记为「已分析未复现」的 #4、#6 已补齐运行时证据（见 0.7 节）。当前状态：

- **已实测复现（5 项）**：#1 静默覆盖、#2 死锁残余场景 B/C/D/E、#4 release 空指针、#5 CString panic、#6 UGC builder panic
- **已实测（工具链，2 项）**：#7 接口覆盖口径核实、#9 测试 7/11/2 + clippy 122 警告
- **已实测后调整定级（2 项）**：#3 resize OOM 降为低风险、#8 Arc 开销性能论据否证
- **对比项（非本仓缺陷，1 项）**：#10 Steamworks.NET `AllocHGlobal`

除 #10 是对比项外，**本仓 9 项缺陷全部有运行时或工具链证据**，无一项仅停留在代码阅读层面。

---

## 11. 总结

实证分析的核心结论是：**steamworks-rs 的问题集中在实现层的可证明缺陷，而非架构落后**。

已实测复现（5 项）：77 处 CString panic（#5）、同类型回调静默覆盖（#1）、死锁残余场景 B/C/D/E（#2）、release 空指针致 SIGILL（#4）、UGC builder 链式 panic（#6，25 处 panic 点；「不可回滚」归因已订正为暂存语义，见 defect-06 档）。经工具链实测证实：测试 55% 不可在 CI 运行（#9）、clippy 122 警告无门禁拦截。实测后调整定级：resize OOM 降为低风险（#3）、**Arc 开销性能论据否证（#8）**、**死锁主场景已由上游 PR #259 修复（#2）**。

**两项重要的负面结论**：

1. **性能论据不成立**。我最初列出的「性能缺陷」（42 处 Arc 克隆）经 1 亿次迭代实测后否证——8ns/次，占帧预算 0.0005%。steamworks-rs 并没有我原先以为的性能问题。
2. **真实短板是正确性与错误处理**：panic 点遍布公开 API（#5 的 77 处、#6 的 builder 链）、release 下空指针直接杀进程（#4）、55% 测试无法在 CI 运行（#9）。这些缺陷的共同特征是「编译期完全不可见」——`cargo build` 干净通过，但运行时行为是 panic 或 SIGILL。

Steamworks.NET 的对比短板仍是性能与维护模式（`AllocHGlobal` 慢、单人占 90% 提交、仓库零 CI workflow）。

SuperSteam 的价值主张需相应收敛：**不是「做一个更快的绑定」**（双方性能都不差），**而是「做一个正确性可证明的绑定」**——以 Rust 类型系统消灭已实证的缺陷，同时补齐必补接口（Inventory/HTTP/ParentalSettings）。

### 关于阶段 1 与上游协作的重要约束

我此前建议阶段 1 的缺陷修复「可直接作为 steamworks-rs 的上游 PR」。**这条建议需两重修正**：

**第一重（纪律层面）**：CI 全绿 ≠ 该合。CODEOWNERS、审批口径、是否接受重构式改动都是维护者的决定，不是技术正确性能替代的。

**第二重（事实层面，本次核查得出）**：**先核查上游状态，再决定是否提 Issue。** 我原计划就缺陷 #2 新开 Issue，核查后发现：

- Issue #253 已存在，且报告者的根因分析正确
- 维护者 james7132（collaborator）与 owner Noxime 均已回应，倾向方案 4
- PR #259 已合并，修复了主场景

**新开 Issue 就是重复报告**，只会浪费维护者时间。正确做法是：先 `gh issue list --search` 检索，若已有对应 Issue，则在现有 Issue 上补充新证据。

就缺陷 #2 而言，可贡献的内容是 #259 **未覆盖**的实测证据：回调内 `register_callback`、嵌套 `run_callbacks`、回调内丢弃 `CallbackHandle` 三种场景仍死锁（`repro/post_fix_deadlock_test.rs`）。这比另开 Issue 有价值得多。

### 已执行：在 #253 补充实测证据

已在 https://github.com/Noxime/steamworks-rs/issues/253#issuecomment-6093232733 发布补充评论（账号 `bfxh`），内容为：

1. 核实 #259 对当前 `master` 的实际效果（主场景确已修复）
2. 报告 B/C/D 三种残余死锁，附 `src/lib.rs:133-140` 当前代码作证（注：E 场景系本轮深挖新发现，尚未追评，待用户确认）
3. 指出维护者倾向的方案 4 可消除 B 和 C，但 **D 在方案 4 下依然存在**（`process_callbacks` 用户同样会在回调内丢弃 handle）
4. 表示愿意提 PR，但先确认方向

发帖前自查了英文表述，修正两处：避免把死锁责任归给 Valve 的 C 模板；对 `Arc<dyn Fn>` 方案标注 `haven't prototyped that against this codebase`（该方案来自本设计文档，未在本仓实测）。

**后续状态待维护者回应**，不由我预判。若收到「不需要」就停手。

---

## 0.7 缺陷 #4 与 #6 的实测复现

这两项此前标记为「已分析，未构造运行时复现」。现已补齐，且 #4 的实测结果比我预想的更严重。

### 缺陷 #4：release 下 null 指针 = Illegal instruction（`repro/null_guard_test.rs`）

镜像 `src/lib.rs:349-358` 的模式（`debug_assert!(!ptr.is_null())` + 后续裸指针解引用），分两种编译模式运行：

| 编译模式 | null 指针时的行为 | 退出码 |
|---------|------------------|--------|
| `debug_assertions=on` | `assertion failed: !ptr.is_null()`，可捕获 panic | 101 |
| `debug_assertions=off`（release） | **Illegal instruction —— 进程被硬件直接终止** | **132** |

**关键证据**：release 模式下 `catch_unwind` **完全没有机会介入**，进程在解引用 null 时被 SIGILL 杀掉。这比我此前描述的「UB」更具体——不是「可能读到垃圾值」的理论风险，而是实测到的进程立即死亡。

`debug_assert!` 的语义本就是「仅 debug 模式生效」，所以这不是 Rust 的坑，而是**用 `debug_assert!` 守卫一个必须检查的条件**这一用法的错误。对 FFI 指针这种必须验证的输入，应当用运行时检查：

```rust
let ptr = unsafe { sys::SteamAPI_SteamFriends_v018() };
let ptr = NonNull::new(ptr).ok_or(SteamError::NotInitialized)?;  // release 同样生效
```

`src/lib.rs`（24 处）与 `src/server/interfaces.rs`（文本 9 处、活代码 8 处，多的那处在 `apps` 的块注释里）共 **33 处** `debug_assert!(!x.is_null())` 文本命中，全部属于此类。（2026-10-16 复测：旧坐标是 lib.rs 的 501-780 行段与 server.rs 单文件，两者已随目录化漂移；尺 = 先剥掉 `/* */` 块再匹配 `debug_assert(!..is_null())`，文本总数与原文一致。）

**计数更正**：本行此前写的是「61 处、其中 23 处 null 检查、lib.rs 17、server.rs 6」，那是拆文件之前的旧尺。按当前源码逐行重测（`grep -c debug_assert src/**/*.rs` 加 `grep -c "debug_assert.*null"`）：

| 类别 | 数量 | 位置 |
|------|------|------|
| 全部 `debug_*assert` | 74 处，其中**活代码 73**（server.rs 有 1 处在 `/* TODO: Buggy currently? */` 注释块里） | lib.rs 24、ugc 35（query 31 + results 2 + mod 2）、server.rs 9（活 8）、networking_types/mod.rs 4、networking_sockets.rs 2 |
| **其中 null 指针检查** | **33 处，活代码 32** | **lib.rs 24、server.rs 9（活 8，被注释的那处是同一个 `apps()` 块）** |
| 其他断言（如 `debug_assert!(ok)` 检查 Steam API 返回值） | 41（74-33，活代码口径同样 41） | 主要在 ugc/query.rs |

lib.rs 的 24 处守卫每一处都紧贴一行接口指针获取（`let x = sys::SteamAPI_Steam*_vNNN();` 下一行即 `debug_assert!(!x.is_null());`），server.rs 活着的 8 处同理。**接口获取点全部有 debug 级守卫**，唯一例外是 `timeline`（`src/lib.rs:739-745`）：它不产 `debug_assert`，而是把空指针存成 `disabled: timeline.is_null()`，运行时判空——这是更安全的写法，不是漏掉的守卫。

> **计数订正（defect-07 深挖时发现）**：此处原写「15 个调用（17 处检查）」，数字自相矛盾——窄正则漏了 `networking_messages`/`networking_sockets`/`networking_utils` 三个用 `_SteamAPI_` 中缀命名的访问器。实际 lib.rs 有 17 个 getter、16 个走 `debug_assert`、`remote_storage` 双指针，合计 17 处。

### #4 的 API 变更讨论：不需要破坏公开签名

先说清影响面的确切口径。23 处是**检查点**，对应的**公开方法**是 23 个：

- `src/lib.rs:349-545` 共 **17 个**：`utils` / `matchmaking` / `matchmaking_servers` / `networking` / `apps` / `friends` / `input` / `user` / `user_stats` / `remote_play` / `remote_storage` / `screenshots` / `ugc` / `timeline` / `networking_messages` / `networking_sockets` / `networking_utils`
- `src/server/interfaces.rs` 今天实测 **8 个**：`ugc` / `http` / `inventory` / `stats` / `utils` / `networking` / `networking_messages` / `networking_sockets`。原文记 6 个（坐标 server.rs:534-593）：其中 `apps` 至今困在块注释里、不是活代码，而 `http` / `inventory` / `stats` 三个是 `a64a7c7` 之后才加的

若按「改成 `Result`」的直觉处理，会变成 `pub fn friends(&self) -> Result<Friends, SteamError>`——**这是对全部 23 个方法的破坏性变更**（原文按 17 + 6 = 23；今天两条分别实测 23 与 8，共 **31** 个 getter），所有调用方都要加 `?`。我在阶段 0 表里把 #4 标为「中风险（改公开 API 返回类型）」就是这个意思。

**但这个代价可以完全避免**，因为同一文件里已有零破坏的现成方案：

```rust
// src/timeline.rs:7 —— timeline 的做法
pub fn timeline(&self) -> Timeline {          // 签名不变，无 Result
    let timeline = sys::SteamAPI_SteamTimeline_v004();
    Timeline {
        timeline,
        disabled: timeline.is_null(),          // 运行时标记，release 同样生效
        _inner: self.inner.clone(),
    }
}
```

配合 `src/timeline.rs:62-65` 的消费方式：

```rust
pub fn set_timeline_game_mode(&self, mode: TimelineGameMode) {
    if self.disabled {
        return;                                 // 接口不可用则静默返回，不解引用
    }
    unsafe { ... }
}
```

**为什么这个方案更优**：

| 维度 | `Result` 方案 | `disabled` 标记方案（timeline 式） |
|------|--------------|-----------------------------------|
| 公开签名破坏 | **23 个方法全变**，调用方全部要改 | **零破坏** |
| release 安全性 | 安全 | **安全**（同样是运行时判断） |
| 调用方负担 | 每个调用点处理 `Err` | 无 |
| 语义准确性 | 优（强制处理不可用） | 较弱（静默降级） |
| 实现成本 | 高（23 处签名 + 全部调用点） | 中（23 个结构体加字段 + 方法入口加判断） |

**关键权衡在语义准确性**：`disabled` 方案是「静默降级」——接口不可用时方法什么都不做就返回，调用方可能误以为操作成功了。而 `Result` 方案强制调用方面对「Steam 接口不可用」这个事实。

但这两种「不准确」的程度不同：`debug_assert!` 的问题是 **release 下进程直接 SIGILL 死掉**，而 `disabled` 方案最坏是「操作没生效」。对一个通常**必然可用**的接口（Steam 客户端版本匹配时），后者的实际发生率远低于前者，且不致命。

**我的建议**：采用 timeline 式方案修 #4，理由是零破坏 + 已在本仓验证可行。但要在文档中明确记录这是「以语义严格性换 API 稳定性」的权衡——若未来某个接口变成「可能不可用且调用方必须知道」，再对该接口单独升级为 `Result`。

这个案例也说明：**先查仓内是否已有同类问题的正确处理方式，能显著降低修复成本**。我最初直接跳到 `Result` 方案，是因为没注意到 `timeline` 已经是另一套做法。

**一个有力的内部对照**：同一文件里的 `timeline` 接口（`src/lib.rs:512-518`）用了**运行时**判空：

```rust
Timeline {
    timeline,
    disabled: timeline.is_null(),   // 运行时检查，release 同样生效
    _inner: self.inner.clone(),
}
```

且 `disabled` 字段在 `src/timeline.rs:64/77/94/116` 被真实消费——接口不可用时返回默认值，**不解引用**。

这说明：**正确的做法在同一文件里已经存在，只是没有统一应用到其余接口获取点**。作者知道该怎么处理指针可用性（Timeline 是可空接口，所以必须处理），但对其余假定「必然可用」的接口用了 `debug_assert!`。问题在于这个假定没有保证——若 Steam 客户端版本不匹配导致某个接口缺失，release 构建就会直接 SIGILL，而 debug 构建只是断言失败。

### 缺陷 #6：builder 链 panic 面（`repro/ugc_builder_panic_test.rs`）

镜像 `src/ugc.rs:933-1050` 的链式模式，在 handle 上记录已提交步骤并实现 `Drop`，实测输出：

```
场景 1：content_path 传不存在的路径
  panicked at ugc_builder_panic_test.rs:50:45:
  called `Result::unwrap()` on an `Err` value: Os { code: 2, kind: NotFound }
    [Drop] handle=1234 被释放，已提交步骤: ["title=\"My Mod\"", "desc=\"A cool mod\""]

场景 2：description 含内嵌 NUL
  panicked at ugc_builder_panic_test.rs:43:36:
  called `Result::unwrap()` on an `Err` value: NulError(4, [...])
    [Drop] handle=5678 被释放，已提交步骤: ["title=\"My Mod\""]
```

**关键证据**：`[Drop]` 行证明 panic unwind 时，前序步骤（`title`、`description`）已通过 `SetItemTitle` 等调用**写入暂存会话**，但：

> **归因订正（第 12 处自我更正，详见 defect-06 文档）**：此处原写「已写入 Steam 侧、无法回滚」——**错**。Valve 头文件 `isteamugc.h:296` 明确 `Set*` 是暂存（"Set changed properties **before commiting** update"），只有 `SubmitItemUpdate` 才 commit。链式 panic 时 `submit` 从未调用，**公开物品状态没有发生任何变更，不存在需要回滚的东西**。原复现输出的「无法回滚」文案是模拟器自设语义，已订正。

订正后的真实危害：

1. **panic 中断链**，无 `Result` 降级路径（25 个 panic 点，见 defect-06 计数）
2. 暂存会话被放弃，且 **SDK 无释放 API**（`isteamugc.h` 全文无 release/abort）
3. 调用方**在编译期没有任何提示**知道这个方法可能 panic（`#[must_use]` 仅覆盖 6/13 方法）

对比同脚本的 `Result` 链模式（场景 3/4）：错误沿链传递，调用方可选择重试、放弃或上报，`Drop` 正常释放无状态泄漏。

`content_path`（src/ugc.rs:986-997）尤其糟——它有**两个** `unwrap()`（`canonicalize` + `CString::new`）加一个 `assert!`，三个失败点挤在四行代码里。

其余缺陷（#1 静默覆盖、#5 CString panic）在提交前同样需先检索是否有重复报告。
