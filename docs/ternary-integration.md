# 三值逻辑能优化这个仓的什么，不能优化什么

> 对象：本仓 `bfxh/steam-sdk-rs`（crate `steamworks` 0.13.1，edition 2021，MSRV 1.80）
> 与 `bfxh/trit`（workspace 0.12.0，edition 2024，MSRV 1.88）。
>
> 口径：关于本仓的每条断言都带 `文件:行号`，由 `scripts/ci/doc_pointers.py` 校验能否落到非空行。
> 数字分三档标注——**【本机实测】**（2026-10-28，rustc 1.99.0，单文件基准，§3.2 给了完整条件）、
> **【trit 读数】**（引自 trit 仓自己的历史测量，负载与本仓不同，不可直接搬）、**【未实测】**。
> 本文只做分析与方案，不含任何代码改动。

---

## 0. 结论

1. **三值在这里优化的是契约，不是吞吐。** 本仓的热路径是 FFI 与 Steam 自己的 IPC；
   把布尔换成三值，在 SDK 的真实规模（好友 ≤ 10³、DLC ≤ 10²、库存项 ≤ 10³）上带来的是
   亚微秒级的差异，观测不到。§3.2 的同机基准把这个量级钉住了。
2. **真正的缺口是"未知"不能组合。** 本仓有 **99 个 `pub fn -> bool`**，而 Steam 自己的 API
   里"答不上来"是常事（未登录、尚未缓存、请求未完成）。现状靠三种互不兼容的手工写法各自处理
   `X`：具名变体、`Option`、以及直接吞成 `bool`。一旦写 `owned && installed`，`X` 就死了。
3. **最值得引 trit 的地方是构建期，不是运行时依赖。** `trit verify`（SAT/BDD）能把
   "未知时绝不放行"这类策略证明成反例可复算的判据；而 `trit-core` 的运行时依赖会带来
   edition 2024 / MSRV 1.88 / 非可选 serde 的代价，与本仓 1.80 的承诺冲突。
4. **运行时已按方案 A 落地**：新增 `tri` 模块（`Ternary` + K3 四操作，零依赖、MSRV 不动）。
   `trit-core` 不引入；仅当确实要用 `TriVec` 的双导轨批量表示时，才考虑可选的 `ternary` feature。
5. **明确不在能力面上**：`trit-dot` / `trit-net` / `trit-gpu` / `trit-attn` / `trit-float` /
   `trit-machines` 与本仓负载无关。把它们接进来只会增加依赖体积，不会带来任何可测收益。

如果只做一件事：**做 §3.1（所有权/许可证的 K3 投影）+ §3.3（构建期策略证明）**，跳过 §3.4 与 §3.2 的批量表示。

---

## 1. trit 有哪些面，哪些和这个仓有关

按"与本仓的相关度"排序，而不是按 trit 自己的重要性：

| trit 组件 | 能力 | 与本仓的关系 | 判断 |
|---|---|---|---|
| `trit-core` | `Ternary{F,X,T}`、K3/Ł3/B3 三套语义、`TriBit`/`TriVec` 双导轨位并行、`#![forbid(unsafe_code)]` | 三值语义与紧凑表示 | **采用其语义，未必要其 crate**（§5） |
| `trit-sat` / `trit-bdd` / `trit-policy` | 构建期穷举 / SAT / canonical BDD 证明，返回反例 | 证明准入策略在 `X` 下不放行 | **最有价值**（§3.3） |
| `trit-codec` | 三值打包（5 trit/字节）、按分布选编码、平衡三进制、GF(3) 纠错 | 权益快照的持久化/网络传输 | 有限，本仓规模下不值得（§3.4） |
| `trit-belnap` | Belnap 四值 `{T,F,B,N}`，`B` 表示"既真又假" | 两个来源冲突（缓存 vs 实时、本地 vs 云） | 窄场景，值得一层（§3.5） |
| `trit-circuit` / `trit-schedule` / `trit-topology` | 电路分析、DAG 调度、可达性证书 | 无对应物 | 不用 |
| `trit-dot` / `trit-net` / `trit-gpu` / `trit-attn` / `trit-float` / `trit-machines` / `trit-texture` | 数值内核、三值网络、GPU、元胞机、纹理 | 与本仓无关 | **明确拒绝**，别 cargo cult |

一个容易被忽略的优点：`trit-core` 整个 crate 是 `#![forbid(unsafe_code)]`。
本仓是 FFI 密集的仓，任何新增的安全逻辑层如果能做到零 `unsafe`，本身就是一条可写进
`docs/SuperSteam-Design.md` 的论据。

---

## 2. 本仓的三值缺口（可复算的证据）

### 2.1 规模

在 `src/` 下用一条命令即可复算：

```bash
grep -rn "pub fn .*-> bool"      --include=*.rs src | wc -l   # 99
grep -rn "pub fn .*-> Option<"   --include=*.rs src | wc -l   # 58
grep -rn "pub fn .*-> Result<"   --include=*.rs src | wc -l   # 145
grep -rnE "^\s*Unknown[,\(]"     --include=*.rs src | wc -l   # 19
```

99 个布尔答案里，`Apps` 一族是重灾区：`src/app.rs:25`（`is_app_installed`）、
`src/app.rs:31`（`is_dlc_installed`）、`src/app.rs:40`（`is_subscribed_app`）、
`src/app.rs:45`（`is_subscribed_from_free_weekend`）、`src/app.rs:50`（`is_vac_banned`）、
`src/app.rs:67`（`is_subscribed`）。`src/parental.rs:76`（`is_app_blocked`）同理。
它们返回 `bool`，而 Steam 在"尚未登录、尚未缓存、查询不适用于当前会话"时给出的同样是 `false`。

> 这一条里"Steam 在离线时返回 false"属于 **【未实测】**：本机没有可复现的 Steam 客户端会话，
> 结论来自 Steamworks 文档与社区口径。但下面三条是**仓内可查**的事实，不依赖任何外部假设。

### 2.2 仓里已经出现过三种互不兼容的 `X`

1. **具名变体**：`src/user.rs:571` 的 `UserHasLicense` 有 `HasLicense` / `DoesNotHaveLicense` /
   `NoAuth` / `Unknown`，这是 Steam 自己的 `EUserHasLicenseForAppResult` 的忠实投影——
   **Steam 本来就在回答三值**（`src/user.rs:185`）。
2. **`Option`**：`src/remote_storage.rs:202` 的 `quota()`，其文档明写
   `None` 表示"Steam 答不上来（云存档关闭）"，而不是"剩余 0 字节"。
3. **直接吞掉**：§2.1 的 99 个 `bool`。

这三种写法各自都对，但**彼此不可组合**。`UserHasLicense::NoAuth` 没法参与布尔表达式，
`quota()` 的 `None` 也没法和别人的 `None` 区分"都是答不上来"还是"一个是 0 字节"。
写一条"能开机甲吗"的判定时，这三种 `X` 不会自然汇成一个 `X`，只会被 `unwrap_or(false)` 抹平。

### 2.3 状态被强行塞进二值通道的典型

`InventoryResult` 有三态：未完成 / 成功 / 失败。类型上只有两态——
`src/inventory.rs:152` 的 `status()` 走 `src/lib.rs:99` 的 `to_steam_result`，
而 `k_EResultPending` 在 `src/error.rs:157` 有具名变体，于是"未完成"被放进了 `Err` 通道。
后果不是 panic（这点是好的），而是调用方必须先用 `src/inventory.rs:141` 的 `is_ready()`
再调 `status()`，**两次调用才能还原一个三态**。任何只写 `if let Err(e) = status()`
的调用方都会把"还在加载"当成错误处理。

### 2.4 仓自己已经有"未知必须可表达"的原则

`scripts/ci/no_panic_on_unknown_enum.py` 的判据是零容忍 `unreachable!()`：
Steam 运行期可能返回比 vendored 头文件更新的枚举值，兜底臂必须落到
`FriendState::Unknown` 这类具名变体（`src/friends/friend.rs:210` 就是这个修法）。
这条门守住的是**枚举层的 `X`**。三值要做的，是把同一条原则从"枚举层"推广到"谓词层"：
`X` 不只是某个枚举的一个变体，而是一个能参与 `∧ ∨ ¬ ⇒` 的值。

顺带说一个同一原则还没覆盖的洞：`src/networking_types/mod.rs:191` 在
`ESteamNetworkingAvailability` 的兜底臂上仍然是 `panic!`。它的 `_[..]` 分支不该活着。

---

## 3. 能落地的地方

### 3.1 落点 A（已落地）：所有权/许可证查询的 K3 投影

**已落地的东西.** 新增 `tri` 模块（实现 139 行、测试 202 行，同一个文件；无 FFI、无 `unsafe`、
无分配），导出：

```rust
#[repr(u8)]
pub enum Ternary { False, Unknown, True }      // 1 字节

impl Ternary {
    pub const fn from_bool(bool) -> Self;
    pub const fn to_bool(self) -> Option<bool>; // Unknown -> None
    pub const fn is_unknown(self) -> bool;
    pub const fn and(self, other: Self) -> Self;      // K3
    pub const fn or(self, other: Self) -> Self;       // K3
    pub const fn implies(self, other: Self) -> Self;  // K3：Unknown ⇒ Unknown = Unknown
}
impl Not for Ternary { .. }                    // !x
impl From<UserHasLicense> for Ternary { .. }   // Has→True, DoesNotHave→False, NoAuth/Unknown→Unknown
```

**没有做的事，以及为什么.** 上一版这里举的例子是给 `Apps::is_subscribed_app` 加一个
`-> Tri` 的伴随方法。落地时否掉了：Steam 的布尔查询没有"答不上来"这个返回值，
包装层若在未登录时凭空返回 `Unknown`，那是**猜测**而不是信号。
`Ternary` 的 `Unknown` 必须来自一个真实来源——`UserHasLicense::NoAuth` 有，`bool` 没有。
所以只有许可证这一处做了投影，其余布尔 API 保持原样：**宁可没有 X，也不要假的 X。**

原签名一行未动，因此是纯加法、semver 兼容。

**为什么必须是 K3，不能是 B3.** 这是本文最有技术含量的一条，且可以在 `trit-core` 的
`algebra.rs` 里逐行对照：

| `a ∧ b` | b=F | b=X | b=T |
|---|---|---|---|
| **a=F** | F | **F**（K3）/ **X**（B3） | F |
| **a=X** | **F**（K3）/ **X**（B3） | X | X |
| **a=T** | F | X | T |

需求是"能不能开机甲"= `owned ∧ installed`。

- **K3** 下 `F ∧ X = F`：用户**确定没买**，此时安装状态未知不影响结论——可以确定地拒绝，
  也能安全地做负缓存。
- **B3**（Bochvar，`X` 是毒值）下 `F ∧ X = X`：把"确定没买"降级成"不知道"，
  最确定的那个信息反而被丢掉了。

所以本仓的默认代数必须是 K3（`trit-core` 的 `Algebra::Kleene`），而不是"随便一个三值就行"。
Ł3 在 `0/1` 上等价于 K3，但它的强加法 `X ⊕ X = T` 是另一套直觉，本仓用不到，也不该误用。

**判据（可判红）.**

1. 9 格真值表测试，逐格断言 K3 的 `∧` / `∨` / `⇒`；把上表里加粗的两格单独写成测试，
   防止有人"优化"成 B3。
2. 一组"离线快照"回放：把 `Tri::Unknown` 注入查询层，断言下游判定返回 `Unknown` 而**不是** `No`。
   这条测试要在旧 `bool` API 上同步跑一份，证明旧 API 确实给出 `No`——否则这条测试没有区分力。
3. 组合律：`owned ∧ installed` 在 `(F, X)` 上等于 `F`，在 `(T, X)` 上等于 `X`。

### 3.2 落点 B（有限）：批量谓词的紧凑表示

**场景.** "我的 1000 个好友里，谁同时在线且拥有 DLC X"、库存里"可交易且未消耗且未移除"
（`src/inventory.rs:141` 一类谓词的批量版）。这类问题的自然表示是每个元素一个三值，
而 Rust 的直觉写法 `Vec<Option<bool>>` 已经是 1 B/元素。

**本机实测（可复跑）.**
`repro/ternary_bulk_bench.rs` 是 std-only 单文件，不引入 criterion，也不进 CI（与本仓
`repro/` 其余文件同一种形态）：

```bash
rustc -O repro/ternary_bulk_bench.rs -o /tmp/tbench && /tmp/tbench
```

注意 `repro/` 被 `.gitignore` 整体排除（本仓把它当取证材料，不进仓，现有的 13 个 repro
文件也一样），所以复跑前这个文件必须在本机存在。

口径：K3 AND，随机数据含 10% `X`；每次测量先预热，随后 **5 轮轮转交错**（每轮把四种实现
换个起点跑），取中位数；计时前后用 `black_box` 挡住优化；输入缓冲区保持不变
（否则第二轮起测的是上一轮的结果）。**先自检再加计时**：K3 真值表 9 格 + 非字对齐长度
10 007 上逐位对拍双导轨与标量，任何一格不符直接退出 1。

双导轨是 `TriVec` 的逐字等价实现（`ones`/`unknowns` 两个 `u64` 平面），`Vec<Ternary>` 用
与 `trit-core::Ternary` 相同的 `#[repr(u8)]` 三变体表。**这是等价实现，不是链接 `trit-core`
本体**（见 §7）。

**表 A：纯 AND**（ns/元素；取一次代表运行。跨运行抖动主要在标量列：三次运行读到
0.65–0.86 ns/元素，其余列波动 < 15%）

| n | 双导轨（2 bit/元素） | `Vec<Ternary>`（1 B/元素） | `Vec<Option<bool>>`（1 B/元素） | 打包二进制 AND（丢掉 `X`） |
|---|---|---|---|---|
| 1 000 | 0.0051 | 0.67 | 0.056 | 0.0026 |
| 100 000 | 0.0089 | 4.39 | 0.055 | 0.0016 |
| 1 000 000 | 0.0105 | 4.41 | 0.061 | 0.0034 |

**表 B：同一遍里统计 True 的个数**（"我这些好友里谁满足"的真实形态）

| n | 双导轨 AND+popcount | 打包二进制 AND+popcount | 比值 |
|---|---|---|---|
| 1 000 | 0.0088 | 0.0068 | 1.30× |
| 100 000 | 0.0131 | 0.0062 | 2.12× |
| 1 000 000 | 0.0144 | 0.0086 | 1.66× |

读法：

- **携带 `X` 的价格是 2–6×（纯 AND），不是 1.x×。** 这修正了本文上一版的估算：那一版把
  `popcount` 也算进了每次重复，计数主导了计时，于是得到"只慢 1.4×"的假象。两种口径必须
  分开报——表 B 才是把计数算进去之后的样子（1.3–2.1×）。
- **比值随 n 非单调**（2.1× → 5.8× → 2.9×），这是缓存层次切换：100 000 档双导轨的 6 个
  平面越过了最舒服的缓存区，而 10⁶ 档两边被同一级缓存按住。**不要引用单一 n 的比值**，
  必须带规模档——这与本仓"性能声明要带同机读数"的规矩一致。
- 对 `Vec<Ternary>` 快 **~250–3 200×**、对 `Vec<Option<bool>>` 快 **~18–36×**。
  解释成"标量枚举在随机数据上分支不可预测、`Option<bool>` 被编译成无分支字节选择"是
  **候选解释，本文没有查汇编验证**。
- **绝对值让所有倍数失效**：n=1000 时表 A 是 5.1 ns / 670 ns / 56 ns / 2.6 ns，
  表 B 是 8.8 ns 对 6.8 ns。本仓一次 FFI 调用就是 10²–10³ ns。
  **没有任何 SDK 热路径会因为这 2–6× 改变。**

存储侧同理：n=10 000 时双导轨 2 512 B、打包二进制 1 256 B、标量 10 000 B。
真实 Steam 库是 10²–10³ 项，也就是 80–250 B。省下来的是"看不见的字节"。

**结论：不要为了快做这件事。** 只有在"需要把一批三值谓词快照下来、并在多个调用点组合"
时，双导轨才值得用；它的价值是可持久化、可组合、且不需要 `Vec<Option<bool>>` 的 1 B/元素。
若只是为了在内存里算一次，用 `Vec<Tri>` 更直白。

### 3.3 落点 C（性价比最高）：构建期策略证明

这是 trit 唯一一个"本仓目前完全做不到"的能力。本仓的测试已经能证明
"每个枚举名各自映射到不同的 Steam 码"（`docs/SuperSteam-Design.md` 里 duration-control 的成对互异测试），
但没有任何手段证明"一条策略在所有三值输入下都不放行"。

策略文件（`trit verify` 的 JSON 形状，示例）：

```json
{
  "algebra": "kleene",
  "definitions": {
    "can_play": "owned & installed & license"
  },
  "assumptions": ["can_play"],
  "assertions": [
    { "name": "no-false-allow", "expr": "owned & license" },
    { "name": "banned-never", "expr": "vac_banned => !can_play" },
    { "name": "parental-never", "expr": "parental_blocked => !can_play" }
  ]
}
```

三个必须说清楚的点：

1. **`assumptions` 是关键，不是装饰.** K3 下 `a ⇒ a` **不是**恒真（`X ⇒ X = X`）。
   所以"`can_play` 蕴含 `owned`"不能写成 `can_play => owned`——它会在两边都 `X` 的赋值上返回 `X` 而不是 `T`，
   判红，且红得没有信息量。正确写法是把 `can_play` 放进 `assumptions`，断言 `owned`：
   查询变成"前提确定为 True 的赋值里，有没有 `owned` 不为 True 的"，这才是"不误放行"的本意。
   这条坑在 trit 仓自己的策略测试里踩过，属于必须写进注释的语义。
2. **它证明的是策略，不是实现.** 证明通过只说明这张表在数学上自洽；
   实现有没有照表写，仍然要靠单元测试。别把构建期证明当成集成测试的替代品。
3. **门必须能判红.** 本仓的规矩是"every gate must prove it can fail"。
   新门除了跑通正例，还必须带一个**故意写错的策略**（比如把 `assumptions` 换回 `can_play => owned`），
   断言 `trit verify` 返回非零并给出反例赋值。没有这一条的门等于没有门。

**成本.** CI 需要 `trit-cli`：`cargo install trit-cli --version 0.11.0 --locked` 或下载 Release 二进制。
它会占掉 CI 的几分钟。建议先放在 `quality-extra.yml`，不要进必需 check，直到门自己稳定。

### 3.4 落点 D（不建议）：快照与消息编码

`trit-codec` 的三档在 trit 仓里已经量过 **【trit 读数】**：

- 全二值（无 `X`）→ 1 bit/trit，比双导轨省 50%；
- 非零率 < 4.85% → 索引编码，1% 非零时省 83%；
- 其余 → 5 trit/字节 = 1.6 bit/trit，省 20%（已贴 `log2 3` 下限）。

放到本仓：10 000 项快照从 2 512 B 降到 1 256 B（全二值）。
**省 1.2 KB，代价是一条编解码依赖和一条新的失败模式**（非规范尾字节、非法字节 243..=255）。
除非将来要往 `networking_messages` 里塞**十万级**的三值快照，否则不值得。
而且 `plan::View` 的随机访问只在"读少数元素"时赢；全量扫描应当整条 `decode`，
这条在 trit 仓里也是实测过的结论，不要凭直觉反着用。

### 3.5 落点 E（窄，但确实存在）：冲突而不是覆盖

`src/remote_storage.rs:276` 的 `LocalFileChangeKind` 只有 `Invalid/Updated/Deleted`，
而"本地和云端都变了"这件事在 Steam 的云同步里是真实存在的状态。同理还有
"缓存说拥有、实时许可证查询说没有"（家庭共享被撤销、退款）。

Belnap 四值的 `B`（both）正好表示"两个来源都声称自己有效"。
建议只在**一个**函数里用：输入是两个来源的三值结论，输出 `{T,F,B,N}`。
不要把它扩散到公开 API——公开面上多一个四值枚举，收益远小于理解成本。

---

## 4. 明确不该做的

1. **不要把 `Result` 换成三值。** 错误不是"未知"。`SteamResult` 要保留，三值只出现在
   "查询的答案"这一层。`Err(Pending)` 的问题是"未完成被塞进错误通道"，
   解法是把状态拆出来（`{Pending, Done(T), Failed(E)}`），不是把 `Err` 变成 `X`。
2. **不要在 FFI 热点和 callback 派发里用枚举级三值。** 每帧调用的 `run_callbacks` 路径上，
   把 `bool` 包成枚举再解包是净成本。真要进热点，只能走 `TriBit::from_binary` 那种
   **零开销**路径（双导轨的 `unknowns` 为 0 时与 `u64` 完全同构），不是 `Ternary` 枚举。
3. **不要把多值状态压成三值。** `NetworkingConnectionState` 有 6 个值
   （`src/networking_types/mod.rs:27`），`Connecting` 和 `FindingRoute` 都是"还没连上"，
   但诊断信息不同。三值只能用在**决策边界**做投影，原始枚举必须完整保留。
4. **不要用 B3 或 Ł3 当默认。** §3.1 的 9 格表就是理由；B3 会把"确定没买"变成"不知道"。
5. **不要碰 Steam 的 ABI 结构体。** `SteamNetConnectionInfo_t` 之类的布局是 C++ 侧的契约，
   任何紧凑编码都只能在**自己拥有的**缓冲区里做。
6. **不要接 `trit-gpu` / `trit-dot` / `trit-net` / `trit-attn`。** 本仓没有对应的数值负载。
   把它们接进来只会让 `cargo package` 变重，并多出编译失败的平台。
7. **不要为"三值更快"做这个改动。** §3.2 的读数已经说明快不过打包二进制，省的是分支和类型安全，
   不是时间。

---

## 5. 依赖与 MSRV：三条路

硬约束是真实的：

| | 本仓 | trit workspace |
|---|---|---|
| edition | 2021 | 2024 |
| rust-version | 1.80（CI 有 MSRV 作业，且要求单一来源声明） | 1.88 |
| 依赖 | `thiserror` 已是必需；`serde` 可选 | `trit-core` 把 `serde` 作为**非可选**依赖 |

三个方案：

- **A（推荐）自写 K3 层.** `enum Tri { No, Unknown, Yes }` + `and/or/not/implies`，
  约 120 行、零依赖、MSRV 不动、`#![forbid(unsafe_code)]` 可继承。
  代价是失去 `TriVec` 的位并行——但 §3.2 已经说明这在本仓不重要。
- **B 可选 feature.** `ternary = ["dep:trit-core"]`，把三值 API 关在 feature 后面。
  MSRV 作业按现有 `image` 特性的先例只测默认特性（本仓 CI 已有这个豁免结构），
  但**默认构建的 MSRV 承诺不变**这一点必须在 README 与 CHANGELOG 里写明。
- **C 只做 dev-dependency.** `trit-cli` 只出现在 CI 与 `dev-dependencies`，
  运行时零依赖。与 §3.3 配套，是三者里对用户影响最小的。

版本现状（别写错）：crates.io 上 `trit-core`/`trit-cli` 最新是 **0.11.0**，
workspace 的 0.12.0 **未发布**；0.3.2–0.7.0 已全部 yank。
所以方案 B/C 若要用 0.12 的任何东西，必须先发版。

---

## 6. 落地状态

| 切片 | 内容 | 状态 | 判据 |
|---|---|---|---|
| S1 | `tri` 模块：`Ternary` + K3 的 `and`/`or`/`implies`/`!` | **已落地** | 9 格真值表 ×3、De Morgan ×18、`size_of::<Ternary>() == 1`；新增 9 条测试 |
| S2 | 许可证投影 `From<UserHasLicense>` | **已落地** | 4 个变体逐一钉住；`NoAuth` 与 `Unknown` 都必须落到 `Unknown` |
| S3 | 准入策略的穷举证明 | **已落地（仓内穷举，不用外部引擎）** | 3^5 = 243 个赋值：`Yes` 恰好 1 个；`license=Unknown` 时绝不 `Yes`；任一确定的 `False` 必为 `False` |
| S4 | `InventoryResult` 拆三态 | **不做** | 见下 |
| S5 | `trit-cli` 进 CI 做策略证明 | **不做** | 见下 |
| S6 | Belnap 冲突判定（原 E） | **不做** | 窄场景，且当前没有调用方 |

**S4 为什么不做.** `status()` 已经返回 `Err(SteamError::Pending)`——"未完成"这个信息**没有丢**，
只是落在错误通道里，调用方可以匹配。加一个 `InventoryOutcome` 是便利，不是修复；
在没有调用方之前，它只是多出来的公开面。

**S5 为什么不做.** 若 CI 依赖 `trit-cli`，就要装一个 0.11.0 的个人 crate（0.3.2–0.7.0 已全部
yank，见 §5），门会因为别人的发版节奏变红。而这里的策略只有 5 个变量，
3^5 = 243 个赋值，仓内穷举一遍不到 1 ms——**穷举就是证明**，不需要外部引擎。
`trit verify` 留作离线复核手段，不进必需 check；等策略变量数超过约 10 个（3^10 ≈ 5.9 万，
穷举开始变笨重）再重新评估。

**变异记录（这些断言确实能判红）.** 两处定向变异，各自跑完即还原，还原后复跑全绿：

1. 把 `and` 的假值臂从"任一为假即假"改成只认 `(F, F)`（Bochvar 方向）→ 4 条测试变红：
   `and_is_the_kleene_table`、`bochvar_would_lose_the_definite_answer`、De Morgan 那条、
   `unknown_never_becomes_yes`；
2. 把 `implies` 的 `(F, _)` 收窄成 `(F, T)` → `implies_is_not_or` 单独变红。

`unknown_never_becomes_yes` 在变异 1 下也会红，说明它不是装饰：策略层的结论确实依赖 K3 的
"假值优先"语义。

---

## 7. 未验证与待拍板

- **§3.2 的基准不是 `trit-core` 本体**：双导轨按 `TriVec` 的逐字语义手写，`Vec<Ternary>`
  用同布局的手写枚举，单机、单种子、两轮；`repro/ternary_bulk_bench.rs` 可复跑，且**表 A/表 B
  是两种口径**，引用时必须带上是哪一种、哪个规模档。若要对 trit 仓背书，应在它的 `criterion`
  基线里用真实 `TriVec` 跑同一张表（本仓不为此引入 criterion）。
- **Steam 的"离线→false"没有在本机复现**：本文的 §2.1 依赖 Steamworks 文档与社区口径。
  §2.2–§2.4 的证据不依赖这一点，是仓内可查的。
- **策略 JSON 只给了形状，没有实跑**：`trit verify` 的 `assumptions` 语义是 trit 0.11/0.12 的行为，
  落地前要用 S4 的正例+反例各跑一次。
- **已定**：走方案 A（自写 K3 层）；策略证明用仓内 3^5 穷举而不是 `trit-cli`；旧 API 一律不动、
  不标 `#[deprecated]`（见 §6 的变异记录与取舍）。
- **仍未验证**：`Ternary` 的 1 字节布局只在单元测试里钉住；跨平台（MSRV 1.80 / 三平台 CI）
  尚未实跑，本轮只在本机 stable 上编译与测试过。
