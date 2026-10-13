# 缺陷 #5 复核：CString panic 点实为 77，非 73（A2 正则陷阱第 2 例）

> 复核触发：缺陷 #7 深挖发现「窄正则漏 `_SteamAPI_` 中缀命名」后，按清单 A2 新纪律
> 对全部存量计数做双口径交叉验证。本节复核缺陷 #5 的 73/83。

## 结论

**panic 点实为 77，主文档的 73 少算了 4 处跨行链式调用。**

| 口径 | 命令 | ugc.rs | utils.rs | 全局 |
|------|------|--------|----------|------|
| 窄（单行正则） | `grep -r "CString::new(.*\.unwrap()\|CString::new(.*\.expect("` | 17 | 2 | **73** |
| 宽（跨行链式） | python 逐行扫描 + 续行判定 | 20 | 3 | **77** |
| 差值 | — | +3 | +1 | **+4** |

4 处被窄正则漏掉的跨行 `expect`：

| 位置 | 代码 | 性质 |
|------|------|------|
| `ugc.rs:1209` | `let cstr = CString::new(tag)\n .expect("…exclude_tag…")` | **panic** |
| `ugc.rs:1222` | `let cstr = CString::new(tag)\n .expect("…require_tag…")` | **panic** |
| `ugc.rs:1243` | `let cstr = CString::new(language)\n .expect("…language…")` | **panic** |
| `utils.rs:320` | `CString::new(s.as_ref())\n .expect("…")` | **panic**（在 `SteamParamStringArray::new` 内） |

4 处均为 `.expect()` 紧跟换行——`.expect()` 与 `.unwrap()` 同为必 panic，按主文档自己的口径
（「73 处带 `unwrap()`/`expect()` 是真 panic 点」）本应计入，只是单行 grep 匹配不到。

## 「正确处理」的 6 处（83 − 77 = 6，此前记为 10）

| 位置 | 写法 | 生产/测试 |
|------|------|----------|
| `ugc.rs:1100` | `.ok()` | 生产（builder submit 的 change_note） |
| `user_stats.rs:386` | `.map_err(\|_\| ())?` | 生产 |
| `user_stats.rs:418` | `.map_err(\|_\| ())?` | 生产 |
| `user_stats.rs:456` | `.map_err(\|_\| ())?` | 生产 |
| `user_stats.rs:496` | `.map_err(\|_\| ())?` | 生产 |
| `user_stats.rs:867` | `assert!(…is_err())` | **测试**（非修复，是断言 NUL 行为） |

主文档称「另 10 处已正确处理」——实为 **6 处**（其中 1 处还是测试断言，非生产修复，故生产侧
真正正确处理仅 5 处）。此前 10 的来源未标注命令，不可复算。

## 完整分类（15 文件，A2 口径可复算）

全部 `CString::new` = **83**（15 文件）；panic = **77**；非 panic = **6**。

| 文件 | panic | 备注 |
|------|-------|------|
| `ugc.rs` | **20**（原 21→20：1100 用 `.ok()`） | 含 builder 链 |
| `matchmaking.rs` | 11 | |
| `server.rs` | 11 | GameServer 侧 |
| `user_stats.rs` | 7（原 12→7：4 处 map_err + 1 测试） | |
| `friends.rs` | 8 | overlay/rich presence |
| `timeline.rs` | 4 | |
| `input.rs` | 4 | |
| `utils.rs` | 3 | 含 `SteamParamStringArray::new` |
| `networking_sockets.rs` | 3 | |
| `networking_types.rs` | 1 | |
| `remote_storage.rs` | 1 | |
| `screenshots.rs` | 1 | |
| `app.rs` | 1 | |
| `user.rs` | 1 | |
| `user_stats/stats.rs` | 1 | |
| **合计** | **77** | |

## 对文档的影响

- 主文档缺陷 #5「73 处」→ **77 处**；「另 10 处已正确处理」→ **6 处（含 1 测试断言）**。
- 缺陷 #5 严重度不变（高）——73→77 只是计数精度，方向与定性均不变。
- 这是清单 A2「正则类计数」的**第 2 例**（第 1 例是 #7 的 `_SteamAPI_` 中缀）：
  窄正则会系统性漏掉**跨行链式**调用，与漏掉**命名变体**是同一失败模式的两个面。
  教训：计数类结论一律双口径（单行 grep + 跨行扫描）交叉验证。

## 可复算命令

```bash
# 窄：73
grep -rn "CString::new(.*\.unwrap()\|CString::new(.*\.expect(" src/ | wc -l
# 全部：83
grep -rn "CString::new(" src/ | wc -l
# 宽：77（跨行链式含 expect）
python count_cstring_panic.py   # steamworks-rs/ 目录，逐行 + 续行判定，实测输出 panic 77 / 非panic 6 / 合计 83
```


## 实施记录（2026-10-10，分支 `fix/cstring-nul-panic` @ `e7f3775`，基点 `6e4a2bc`，未推送）

**落地形态**：16 文件 +1637/−1283。`pub(crate) fn cstring(s) -> Option<CString>`（lib.rs）+ 按返回通道零破坏降级（不 Result 化，不破签名）：

| 返回通道 | 降级行为 |
|---|---|
| unit | 静默跳过 FFI 调用（加行为注） |
| bool | false（Steam 自身对非法参数的答案） |
| Option | None |
| Result | 复用既有 Err 变体 |
| sys 句柄 | 0（AuthTicket(0) 等） |
| 结构体名字段 | CString::default()（空名，后续查找落空） |
| 回调 API（find_leaderboard / find_or_create_leaderboard） | cb(Err(SteamError::IOFailure)) 后返回——回调承诺不丢，调用方不会挂起 |
| init（Server::init） | Err(SteamAPIInitError::Generic("version string contains a NUL byte")) |
| 一次性 sys 调用点 | c"..." 字面量（编译期 NUL 检查） |

**台账勘误（NetConnection::drop）**：`networking_sockets.rs:1076` 的 CString::new("Handle was dropped").unwrap() 原归「生产 unwrap 点」，实为**字面量输入**（不可能含 NUL）——按最小侵入改为 c 字面量常量化（编译期查 NUL），不引入降级分支。

**测试崩溃机制与修复（本次实施最大教训）**：新增测试构造并丢弃 Inner{manager: Manager::Client} 会触发 `impl Drop for Manager`（lib.rs）→ 在从未初始化的 context 上执行 SteamAPI_Shutdown()；与 server::test（GameServer init 本机可成功）的真实 context 竞态 → 整个测试进程间歇 0xc0000005。证据链：fix 树 8/15 跑崩 vs 基线 0/14；`--test-threads=2 --no-capture` 定位崩溃尾随 test_allocate_and_free_message + Steam SDK stderr「CFrameFunctionMgr::Deregister: function to deregister not found」；线程密度/分配噪声/单个测试逐项否证。修法 = 两个测试 helper 各 std::mem::forget(Arc::clone(&inner)) 泄一个强引用；修后并行 10/10 干净。⚠️ wt-fix-null 的 #4 测试（3 个）有同款隐患，待独立提交补同款修复。

**门禁（同尺 vs HEAD 6e4a2bc，stash roundtrip，共享 target dir，env DEBUG=0 INCREMENTAL=0）**：
- fmt --check exit 0；build exit 0
- clippy kind-diff **0/0**——首跑引入 8 条已全部修掉：question_mark ×3 → `cstring(key)?`（friends 507 / matchmaking 157·252）、manual_unwrap_or_default ×2 → `cstring(name).unwrap_or_default()`（remote_storage 114 / user_stats 654）、needless_question_mark → screenshots 去 Some 包装、drop_non_drop → `let _entry =`（NetworkingConfigEntry 不实现 Drop）、arc_with_non_send_sync → 测试点局部 allow
- 串行 `--test-threads=1`：19 通过 / 11 失败 / 2 忽略，与基线逐字同（11 失败全是本机 NoSteamClient 环境性，两树同集）
- 并行 10×：0 segv

**剩余 CString::new 账**：83 − 58（转化：47 脚本 + 11 手工）= 25 = 19 UGC builder（归 #6）+ 4 map_err + 2 .ok()/helper + 2 test assert。
