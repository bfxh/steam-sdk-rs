# 缺陷 #7 深度分析：接口覆盖——分母 29 本身漏数，宽口径 34/23/11（缺口 11 是口径不变量）

> 依据核查清单完成：A2 计数可复算（双口径）✓ / A5 上游时效 ✓ / C1 重复检索 ✓
> 行号对照工作区（见主文档头部基准声明）。本缺陷为覆盖度统计，无 panic/UB，不适用 A1 复现。

## 结论

**缺陷成立（覆盖率确实低），严重度维持「中」，但主文档的统计有连锁错误**：

| # | 主文档表述 | 实测 | 性质 |
|---|-----------|------|------|
| 1 | 29 个接口 | ⚠️ 宽口径 **34** | 窄正则漏了 5 个 `_SteamAPI_` 中缀命名（NetworkingMessages/Sockets/Utils + GameServer 侧同名 2 个） |
| 2 | 已封装 14 | ⚠️ 宽口径 **23**（窄 18） | 漏算 GameServer 侧与 `_SteamAPI_` 中缀接口 |
| 3 | 缺失 15 | ✅ 实为 **11** | **11 是口径不变量**：宽窄两口径下缺口清单完全相同 |
| 4 | 缺口含 AppTicket/GameCoordinator/DualSense | ⚠️ 三者无独立访问器 | 口径混入，已从缺口表移除 |

**最重要的稳健性发现：缺口 11 在两种口径下完全一致**——宽口径多出的 5 个接口（NetworkingMessages、NetworkingSockets、NetworkingUtils、GameServerNetworkingMessages、GameServerNetworkingSockets）恰好**全部已封装**。所以「必补 Inventory/HTTP/ParentalSettings」的方向结论不受口径选择影响。

---

## A. 取证过程（A2 双口径）

### 口径 1：窄（主文档隐含口径，`SteamAPI_Steam*_v*`）

```bash
grep -rhoE "SteamAPI_Steam[A-Za-z]+_v[0-9]+" steamworks-sys/src/windows_bindings.rs \
  | sed -E 's/_v[0-9]+$//' | sort -u | wc -l   # 29
```

### 口径 2：宽（含 `_SteamAPI_` 中缀命名）—— 推荐

```bash
grep -rhoE "SteamAPI_Steam[A-Za-z]+(_SteamAPI)?_v[0-9]+" steamworks-sys/src/windows_bindings.rs \
  | sed -E 's/_v[0-9]+$//; s/_SteamAPI$//' | sort -u | wc -l   # 34
```

窄口径漏掉的 5 个（命名模式不同：`SteamAPI_SteamNetworkingMessages_SteamAPI_v002` 而非 `SteamAPI_SteamNetworkingMessages_v002`）：

| 接口 | 访问器 | 封装状态 |
|------|--------|---------|
| NetworkingMessages | `SteamAPI_SteamNetworkingMessages_SteamAPI_v002`（`lib.rs:521`） | ✅ |
| NetworkingSockets | `SteamAPI_SteamNetworkingSockets_SteamAPI_v012`（`lib.rs:532`） | ✅ |
| NetworkingUtils | `SteamAPI_SteamNetworkingUtils_SteamAPI_v004`（`lib.rs:543`） | ✅ |
| GameServerNetworkingMessages | 同模式 | ✅ |
| GameServerNetworkingSockets | 同模式 | ✅ |

**计数教训**：Valve 的 flat API 命名不统一（大部分 `SteamAPI_SteamX_vN`，networking 三件套插了 `_SteamAPI_`），单条正则必漏。原分析与我最初的深挖都踩了同一个坑——宽窄双口径交叉验证才暴露。

### 分子：宽 23 = Client 17 + GameServer 6

```bash
grep -rhoE "sys::SteamAPI_Steam[A-Za-z]+(_SteamAPI)?_v[0-9]+" src/ | wc -u  # 23
```

- **Client 17**：utils, matchmaking, matchmaking_servers, networking, apps, friends, input, user, user_stats, remote_play, remote_storage, screenshots, ugc, timeline, networking_messages, networking_sockets, networking_utils（= lib.rs 全部 getter，与 getter 对账一致）
- **GameServer 6**：GameServer, GameServerUGC(`server.rs:536`), GameServerNetworking, GameServerUtils, GameServerNetworkingMessages, GameServerNetworkingSockets

### 缺口：11（宽窄一致）

```bash
comm -23 <(全部访问器) <(src 已封装)   # 宽口径
# Controller, GameServerHTTP, GameServerInventory, GameServerStats,
# HTMLSurface, HTTP, Inventory, Music, ParentalSettings, Parties, Video
```

| 缺口接口 | 虚方法数（严/宽） | 定性 |
|---------|------------------|------|
| ISteamInventory | **30 / 38** | **必补（高）**——内购/开箱 |
| ISteamHTTP | 25 | **必补（高）**——自建后端 |
| ISteamParentalSettings | 6 | **合规必需（高）** |
| ISteamGameServerHTTP | 25 | GameServer 必需（中） |
| ISteamGameServerInventory | 30 | GameServer 必需（中） |
| ISteamGameServerStats | ~10 | GameServer 必需（中） |
| ISteamHTMLSurface | 37 / 39 | 锦上添花（中） |
| ISteamParties | **12**（原「?」） | 锦上添花（中） |
| ISteamMusic | 9 | 可选（低） |
| ISteamVideo | 4 | 可选（低） |
| ISteamController | 34 | 已被 Input 取代（主动不做） |

**方法数双口径**：严=单行 `virtual … = 0;`，宽=含跨行声明的 `virtual` 关键字计数。Inventory 30/38、HTMLSurface 37/39——主文档原写 38/39 用宽口径但未声明；Parties 原为「?」，实测 12。

### 口径净化：三个无访问器接口

`ISteamAppTicket`/`ISteamGameCoordinator`/`ISteamDualSense` 在绑定文件中**无 `SteamAPI_Steam*` 符号**（经 ISteamClient 或并入 Input），不属 34 口径，已从缺口表移除。

---

## C1. 重复检索 / A5. 上游时效（最重要的新发现）

**PR #205「ISteamInventory get_all_items, get_result_items, SteamInventoryResultReady, SteamInventoryFullUpdate」（jstroh，2024-09-09，CLOSED 未合并）**：

- 作者实测：app 480 + 自有 app 2957110、有物品账号，工作正常
- 维护者 **ceifa 两条反馈**：
  1. build 挂了（rustfmt.toml 相关）
  2. **API 风格**：异步结果应照 UGC 的 call_result 包装写（`ugc.rs:574` 风格），不要暴露裸回调
- `mergeable: MERGEABLE`——不是冲突关闭，是**无人推进**（作者未再响应格式问题）

**这改变了 Inventory 缺口的定性**：不是「没人做过」，是「**做过、已实测、维护者已表态 API 偏好、然后停滞**」。直接指导：

1. 补 Inventory **不要从零设计**——PR #205 是唯一经真实账号实测的实现，先读其 diff
2. 上游 PR 必须按 ceifa 偏好：异步结果走 `register_call_result` 包装（C2：维护者需求已部分确认）
3. 补齐成本比主文档预估**低**：有可复用实现 + API 偏好已明确

其它检索结果（#334 legacy auth、#300 构建、#348 glyphs、#293 shutdown、#275 FileShare）均与本缺陷无关。

---

## 对主文档的修正要求（D2）

| 位置 | 原文 | 应改为 |
|------|------|--------|
| 摘要表 #7 | 接口覆盖 14/29 | **23/34**（Client 17 + GameServer 6；缺口 11） |
| 三者对比表 | 14/29 | 23/34 |
| 6.1 节标题 | 封装 14 个 | 封装 23 个（宽口径 34） |
| 6.1 缺失表 | GameServer*(…/UGC/Networking/Utils) | GameServer*(HTTP/Inventory/Stats)——其余已封装 |
| 6.1 缺失表 | AppTicket/GameCoordinator/DualSense 三行 | 移除（无访问器，不属口径） |
| 6.2 节 | 子接口（HTTP/Inventory/Stats/UGC）缺失 | 已覆盖 6 个 GameServer 侧访问器，缺 HTTP/Inventory/Stats |
| Parties ? | ? | 12 |
| defect-04 文档 | 15 个接口 getter | 17 个（timeline 0 assert=disabled 先例、remote_storage 2 assert，合计 17 处判空） |

---

## 深挖产出的新事实

| # | 新事实 | 证据 | 影响 |
|---|--------|------|------|
| 1 | **窄正则漏 5 个 `_SteamAPI_` 中缀接口**，分母应为 34 | bindings 双口径 grep | 主文档 29 是错的；我首版深挖也踩同一坑（双口径交叉才暴露） |
| 2 | 分子 23（非 14 非 18） | src 宽口径 grep + getter 对账 | 覆盖率修正 |
| 3 | **缺口 11 是口径不变量** | comm 对比：宽窄缺口清单逐项相同 | 必补三缺口的方向结论稳健 |
| 4 | GameServer 侧已封装 6 个（非 0 非 4） | server.rs + networking_* 服务器访问器 | 6.2 节「仅主接口」错误 |
| 5 | **Inventory 有停滞已测实现 PR #205** | `gh pr view 205`：MERGEABLE、ceifa 已提 API 风格要求 | 补齐成本低于预估 |
| 6 | lib.rs 实有 17 个 getter（非 15）；timeline 是唯一 0-assert getter（disabled 先例） | 逐 getter awk 对账 | defect-04 计数需订正；disabled 先例的唯一性坐实 |
| 7 | AppTicket/GameCoordinator/DualSense 无访问器符号 | bindings 无对应 SteamAPI_Steam* | 缺口表口径净化 |

**严重度维持「中」**：覆盖率低的方向不变；Inventory 有可复用实现使实际补齐成本低于预估。
