[readme.md.md](https://github.com/user-attachments/files/33299013/readme.md.md)
# Wish Vault · 链上心愿存证

> 一个基于 **Solana + Anchor** 的链上「心愿存证」程序（Program），把用户提交的心愿文本永久写入链上 PDA 账户，形成不可篡改的时间戳存证。
>
> 核心源码：`lib.rs`（约 140 行），程序模块名 `wish_vault`。

---

## 目录

- [1. 项目简介](#1-项目简介)
- [2. 核心功能](#2-核心功能)
- [3. 技术栈](#3-技术栈)
- [4. 账户模型与 PDA 设计](#4-账户模型与-pda-设计)
- [5. 指令详解](#5-指令详解)
- [6. 错误码](#6-错误码)
- [7. 典型调用流程](#7-典型调用流程)
- [8. 工程结构](#8-工程结构)
- [9. 构建、测试与部署](#9-构建测试与部署)
- [10. 设计要点与安全性](#10-设计要点与安全性)
- [11. 已知限制与演进方向](#11-已知限制与演进方向)
- [12. 前端集成速查](#12-前端集成速查)

---

## 1. 项目简介

Wish Vault 让任何人以「钱包即身份」的方式，把自己的一条心愿（一段最长 200 字节的文本）写进 Solana 区块链：

- **永久存证**：心愿望文本、作者钱包地址、链上时间戳（`unix_timestamp`）一并落盘到链上账户；
- **无需后端**：没有数据库、没有服务器，所有状态都在链上账户里，靠 PDA 派生地址寻址；
- **一人多愿**：每个用户拥有一个计数账户，每条心愿按「序号」派生出**唯一且可预测**的账户地址，天然防重、天然可遍历；
- **公开/私密标记**：提交时可标记 `is_public`，供客户端决定是否在心愿墙展示（链上数据本身始终可查）。

一句话概括：**这是一个"区块链许愿池 + 时间戳存证"的最小可用（MVP）示例程序。**

---

## 2. 核心功能

| # | 功能 | 对应指令 | 说明 |
|---|------|----------|------|
| 1 | 初始化作者账户 | `init_author` | 每个钱包只需调用一次，创建该用户的心愿计数 PDA |
| 2 | 提交心愿 | `create_wish` | 校验内容长度与非空，写入心愿 PDA，并让计数 +1 |
| 3 | 内容校验 | — | 非空校验、最大 200 字节校验，防止账户溢出与垃圾数据 |
| 4 | 链上日志 | — | 通过 `msg!` 输出作者、内容、时间戳，便于浏览器/客户端检索 |
| 5 | 确定性寻址 | — | 全部账户由 seeds 派生，客户端可离线推导出任意心愿的地址 |

能力边界（当前版本**不包含**）：修改、删除、关闭账户退款、点赞/打赏、查询指令、权限控制列表。详见[第 11 节](#11-已知限制与演进方向)。

---

## 3. 技术栈

| 层次 | 选型 |
|------|------|
| 区块链 | Solana |
| 智能合约框架 | Anchor（`anchor_lang::prelude::*`，`#[program]` / `#[account]` / `#[derive(Accounts)]` / `#[error_code]` 宏） |
| 语言 | Rust（edition 2021 风格，Anchor 0.2x+） |
| 程序 ID | `declare_id!("3BurprFWn8uhdbQ2bTvd3AwJXNojteCG2wseJvRJezDz")`（占位值，首次 `anchor build` 后会被替换为真实 Program ID） |
| 测试/客户端 | 通常配套 `@coral-xyz/anchor`（TypeScript）或 `anchor-client`（Rust） |

---

## 4. 账户模型与 PDA 设计

本项目共有 **2 种链上账户**，全部为 PDA（Program Derived Address），由程序自己派生和签名，不需要私钥。

### 4.1 `WishAccount` —— 心愿存证账户（每条心愿一个）

```rust
#[account]
pub struct WishAccount {
    pub author: Pubkey,   // 作者钱包地址              (32)
    pub content: String,  // 心愿文本，4 字节长度前缀 + 内容 (4 + 200)
    pub is_public: bool,  // 是否公开展示              (1)
    pub timestamp: i64,   // 链上时间戳（秒）           (8)
    pub wish_index: u64,  // 该用户的第几条心愿         (8)
    pub bump: u8,         // PDA bump 种子             (1)
}
```

| 项目 | 值 |
|------|----|
| Seeds | `[b"wish", author.key().as_ref(), &author_wishes.total_wishes.to_le_bytes()]` |
| 空间 `LEN` | `8 + 32 + 4 + 200 + 1 + 8 + 8 + 1 = 262` 字节 |
| 生命周期 | `init`（由 `create_wish` 创建，`payer = author`） |
| 说明 | 8 字节为 Anchor 账户判别符（discriminator）；`String` 在 Anchor 中按 `4 字节长度 + 内容` 存储 |

### 4.2 `AuthorWishes` —— 作者心愿计数账户（每个用户一个）

```rust
#[account]
pub struct AuthorWishes {
    pub author: Pubkey,      // 作者钱包          (32)
    pub total_wishes: u64,   // 已提交心愿总数     (8)
    pub bump: u8,            // bump 种子        (1)
}
```

| 项目 | 值 |
|------|----|
| Seeds | `[b"author_wishes", author.key().as_ref()]` |
| 空间 `LEN` | `8 + 32 + 8 + 1 = 49` 字节 |
| 生命周期 | `init`（由 `init_author` 创建，`payer = author`） |
| 说明 | 既是「用户档案」，也是心愿账户序号 `wish_index` 的来源 |

### 4.3 地址推导关系

```
作者钱包 (Pubkey)
   │
   ├─ seeds: ["author_wishes", author]  ─────────►  AuthorWishes{ total_wishes }
   │                                                        │
   └─ seeds: ["wish", author, total_wishes_le_u64] ─────────┴──►  WishAccount #0
                                                        │
                                                        └────────►  WishAccount #1 ... 依次递增
```

**要点**：心愿账户地址由「作者 + 当前计数」共同决定，因此

- 序号从 `0` 开始逐个递增，**不存在地址冲突**；
- 客户端只要知道作者地址，就能推算出第 N 条心愿的账户地址；
- 计数与真实账户一一对应，可用来枚举某用户的全部心愿。

---

## 5. 指令详解

### 5.1 `init_author` —— 初始化作者账户

```rust
pub fn init_author(ctx: Context<InitAuthor>) -> Result<()>
```

| 项目 | 说明 |
|------|------|
| 账户 | `author`（签名者，可变，付租金）、`author_wishes`（PDA，`init`）、`system_program` |
| 效果 | 创建 `AuthorWishes` 账户，写入 `author`、`total_wishes = 0`、`bump` |
| 日志 | `作者账户已初始化: {author}` |
| 前置条件 | 无（每个钱包调用一次即可；账户已存在时会因 `init` 而失败） |

**必须先调用此指令**，否则 `create_wish` 会因为找不到 `author_wishes` 账户而失败。

### 5.2 `create_wish` —— 提交一条心愿

```rust
pub fn create_wish(
    ctx: Context<CreateWish>,
    content: String,
    is_public: bool,
) -> Result<()>
```

| 项目 | 说明 |
|------|------|
| 参数 | `content`：心愿文本；`is_public`：是否公开展示 |
| 账户 | `author`（签名者，可变，付租金）、`wish_account`（PDA，`init`）、`author_wishes`（PDA，`mut`，用 `bump` 复用已有账户）、`system_program` |
| 校验 1 | `content` 字节长度 ≤ `WishAccount::MAX_CONTENT_LENGTH`（200），否则 `ContentTooLong` |
| 校验 2 | `content` 非空，否则 `ContentEmpty` |
| 写入 | `author` / `content` / `is_public` / `timestamp`（`Clock::get()?.unix_timestamp`）/ `wish_index`（= 当前 `total_wishes`）/ `bump` |
| 副作用 | `author_wishes.total_wishes += 1` |
| 日志 | `心愿已上链！作者: {author}`、`内容: {content}`、`时间戳: {timestamp}` |

时序要点：`wish_index` 取的是**自增之前**的 `total_wishes`，与 seeds 中使用的值一致，所以第 1 条心愿的 `wish_index = 0`、账户 seeds 中的序号也是 `0`。

---

## 6. 错误码

```rust
#[error_code]
pub enum WishError {
    #[msg("心愿内容不能为空")]
    ContentEmpty,
    #[msg("心愿内容过长，最多200个字节")]
    ContentTooLong,
}
```

| 错误 | 触发条件 | 客户端提示建议 |
|------|----------|----------------|
| `ContentEmpty` | `content` 为空字符串 | 提示用户先写下心愿 |
| `ContentTooLong` | `content` 的 UTF-8 字节数 > 200 | 中文约 66 个字符上限，需在 UI 侧先行统计字节数 |

> 提示：Anchor 的错误码由枚举顺序决定（`ContentEmpty = 6000`、`ContentTooLong = 6001`），客户端可用 `program.error` 映射表还原可读信息。

---

## 7. 典型调用流程

```
① 初始化（每个钱包一次）
   用户钱包 ──init_author()──► 创建 AuthorWishes PDA  (total_wishes = 0)

② 提交心愿（可重复）
   用户钱包 ──create_wish(content, is_public)──► 创建第 N 个 WishAccount PDA
                                                  ├─ 写入内容 / 时间戳 / 序号
                                                  └─ total_wishes: N → N+1

③ 读取（只读 RPC，无需签名，无需本项目提供指令）
   客户端 ──getProgramAccounts(filters: 按 author / discriminator)──► 心愿列表 + 心愿墙
```

---

## 8. 工程结构

`lib.rs` 只是一个 Anchor 工程的**程序主体文件**，放到完整工程里时典型布局如下：

```
wish-vault/
├── Anchor.toml                 # 集群、钱包、测试脚本配置
├── Cargo.toml                  # workspace 定义
├── package.json                # 客户端/测试依赖（可选）
├── programs/
│   └── wish-vault/
│       ├── Cargo.toml          # 依赖 anchor-lang
│       └── src/
│           └── lib.rs          # ★ 本项目源码（指令 + 账户 + 错误码）
├── tests/
│   └── wish-vault.ts           # 集成测试（init_author / create_wish / 长度校验）
└── migrations/
    └── deploy.ts               # 部署脚本
```

`lib.rs` 内部的组织顺序：

| 区块 | 内容 |
|------|------|
| 头部 | `use anchor_lang::prelude::*;` + `declare_id!`（程序地址） |
| `#[program]` 模块 | 业务入口：`create_wish`、`init_author` |
| 账户结构 | `WishAccount`、`AuthorWishes`（含 `LEN` 常量与 `MAX_CONTENT_LENGTH`） |
| 指令账户验证 | `InitAuthor`、`CreateWish`（`#[derive(Accounts)]`，声明签名者、可变性、seeds、空间） |
| 错误码 | `WishError` |

---

## 9. 构建、测试与部署

### 9.1 环境准备

```bash
# Rust + Solana CLI + Anchor CLI
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"   # Solana CLI
cargo install --git https://github.com/coral-xyz/anchor avm --locked
avm install latest && avm use latest                            # Anchor CLI
```

### 9.2 常用命令

```bash
anchor build          # 编译程序，并在 target/idl 下生成 IDL / TypeScript 类型
anchor test           # 启动本地验证者 + 跑 tests/ 下的集成测试
solana-test-validator # 手动起本地链（可选）
anchor deploy         # 部署到 Anchor.toml 中配置的集群
```

### 9.3 关于 Program ID

源码中的程序地址是占位值：

```rust
declare_id!("3BurprFWn8uhdbQ2bTvd3AwJXNojteCG2wseJvRJezDz"); // 首次 build 后会自动替换为真实 Program ID
```

标准流程：`anchor keys list` 取得真实地址 → 同步更新 `lib.rs` 的 `declare_id!` 和 `Anchor.toml` 的 `[programs.localnet]` → 重新 `anchor build`。

### 9.4 编译前自检

- 首行导入应为 `use anchor_lang::prelude::*;`（确认 `use` 关键字与文件末尾没有多余残留字符）；
- 文件末尾的 `#[error_code]` 枚举需以 `}` 正常闭合。

---

## 10. 设计要点与安全性

| 设计 | 价值 |
|------|------|
| **全部使用 PDA** | 账户地址由程序派生，无独立私钥，只有本程序能签名修改，杜绝外部伪造账户 |
| **`init` + `payer = author`** | 由提交者支付租金，程序不承担成本，也无法被薅租金 |
| **seeds 内含作者公钥 + 计数** | 天然隔离不同用户的数据；同一用户的心愿序号唯一、不可覆盖 |
| **复用 `author_wishes.bump`** | `create_wish` 中使用已存 bump 校验计数账户，避免重复推导带来的不确定性 |
| **长度上限常量** | `space` 与校验共用 `MAX_CONTENT_LENGTH`，账户大小固定、不可被撑爆 |
| **链上时间戳** | 时间取自 `Clock`（验证者共识时间），而非客户端传入，无法被本地篡改 |
| **签名者约束** | 两个指令都要求 `Signer`，只有私钥持有者能为自己创建账户 |
| **`mut` 精确声明** | 只有确实需要改动的账户声明为 `mut`，减少不必要的写锁 |

> 需要理解的一点：`is_public` 只是**链上标记**，链上数据对所有人可见，它控制的是客户端是否在「心愿墙」展示，而不是访问权限。

---

## 11. 已知限制与演进方向

| 现状 | 建议演进 |
|------|----------|
| 无修改/删除指令 | 增加 `update_wish`（内容修订留痕）与 `close_wish`（关闭账户，租金退还作者） |
| 无查询指令 | 客户端用 `getProgramAccounts` + memcmp 过滤（按 `author`、`is_public`）；数据量大时可加 `emit!` 事件 + 链下索引器（如 Helius DAS / Solana 索引服务） |
| 日志用 `msg!` | 改为 `emit!(WishCreated {...})` 事件，便于链下订阅与索引，且更省计算预算 |
| `total_wishes` 无上限、单调递增 | 可加统一上限或分页结构；用户级心愿数统计也可迁移到独立的索引账户 |
| 无资金/激励逻辑 | 可加入押注、打赏（SOL/SPL Token）、达成愿望后领取奖励等玩法 |
| 无权限/审核机制 | 需要内容治理时可引入管理员角色、举报与下架标记 |
| 中文长度体验 | 200 字节 ≈ 66 个汉字，客户端应实时显示「剩余字节」，避免用户写满被拒 |
| 无版本/迁移策略 | 上线前固定 `LEN`，后续升级需考虑 Anchor 的账户迁移（`realloc`）方案 |

---

## 12. 前端集成速查

```ts
// 1. 推导 PDA 地址
import { PublicKey } from "@solana/web3.js";

const [authorWishesPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("author_wishes"), wallet.publicKey.toBuffer()],
  programId
);

const [wishPda] = PublicKey.findProgramAddressSync(
  [
    Buffer.from("wish"),
    wallet.publicKey.toBuffer(),
    new (require("bn.js"))(totalWishes).toArrayLike(Buffer, "le", 8),
  ],
  programId
);

// 2. 调用
await program.methods.initAuthor().accounts({ author: wallet.publicKey }).rpc();
await program.methods
  .createWish("愿世界和平", true)
  .accounts({ author: wallet.publicKey, authorWishes: authorWishesPda })
  .rpc();

// 3. 读取某用户的全部心愿
const wishes = await program.account.wishAccount.all([
  { memcmp: { offset: 8, bytes: wallet.publicKey.toBase58() } }, // 8 = discriminator 之后
]);
```

> `offset: 8` 表示跳过 8 字节账户判别符，直接按 `author` 字段过滤。

---

## 许可

未附带许可证文件；如需开源发布，建议补充 `LICENSE`（例如 MIT 或 Apache-2.0）。
