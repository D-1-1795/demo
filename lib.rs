use anchor_lang::prelude::*;

declare_id!("JA3VdoBdSxFW3gGhgeCYPZW8iJ8CFsNa5FuLdEsCafQA"); // 首次build后会自动替换为真实Program ID

#[program]
pub mod wish_vault {
    use super::*;

    /// 提交一条心愿，写入链上PDA账户
    pub fn create_wish(
        ctx: Context<CreateWish>,
        content: String,
        is_public: bool,
    ) -> Result<()> {
        // 限制心愿文本长度，防止账户溢出
        require!(
            content.as_bytes().len() <= WishAccount::MAX_CONTENT_LENGTH,
            WishError::ContentTooLong
        );
        require!(!content.is_empty(), WishError::ContentEmpty);

        let wish = &mut ctx.accounts.wish_account;
        let clock = Clock::get()?;

        wish.author = ctx.accounts.author.key();
        wish.content = content.clone();
        wish.is_public = is_public;
        wish.timestamp = clock.unix_timestamp;
        wish.wish_index = ctx.accounts.author_wishes.total_wishes;
        wish.bump = ctx.bumps.wish_account;

        // 更新用户的心愿计数
        let author_wishes = &mut ctx.accounts.author_wishes;
        author_wishes.total_wishes += 1;

        msg!("心愿已上链！作者: {}", wish.author);
        msg!("内容: {}", content);
        msg!("时间戳: {}", wish.timestamp);

        Ok(())
    }

    /// 初始化用户的心愿计数账户（每个用户只需调用一次）
    pub fn init_author(ctx: Context<InitAuthor>) -> Result<()> {
        let author_wishes = &mut ctx.accounts.author_wishes;
        author_wishes.author = ctx.accounts.author.key();
        author_wishes.total_wishes = 0;
        author_wishes.bump = ctx.bumps.author_wishes;
        msg!("作者账户已初始化: {}", author_wishes.author);
        Ok(())
    }
}

// ============ 账户结构定义 ============

/// 心愿存证账户（PDA，每个心愿一个）
#[account]
pub struct WishAccount {
    pub author: Pubkey,       // 作者钱包地址 (32)
    pub content: String,      // 心愿文本 (4 + 200)
    pub is_public: bool,      // 是否公开 (1)
    pub timestamp: i64,       // 链上时间戳 (8)
    pub wish_index: u64,      // 该用户第几条心愿 (8)
    pub bump: u8,             // PDA bump (1)
}

impl WishAccount {
    pub const MAX_CONTENT_LENGTH: usize = 200;
    // 8 discriminator + 32 + 4+200 + 1 + 8 + 8 + 1
    pub const LEN: usize = 8 + 32 + 4 + 200 + 1 + 8 + 8 + 1;
}

/// 作者心愿计数账户（PDA，每个用户一个）
#[account]
pub struct AuthorWishes {
    pub author: Pubkey,       // 作者钱包 (32)
    pub total_wishes: u64,    // 已提交心愿总数 (8)
    pub bump: u8,             // bump (1)
}

impl AuthorWishes {
    pub const LEN: usize = 8 + 32 + 8 + 1;
}

// ============ 指令账户验证 ============

#[derive(Accounts)]
pub struct InitAuthor<'info> {
    #[account(mut)]
    pub author: Signer<'info>,

    #[account(
        init,
        payer = author,
        space = AuthorWishes::LEN,
        seeds = [b"author_wishes", author.key().as_ref()],
        bump,
    )]
    pub author_wishes: Account<'info, AuthorWishes>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateWish<'info> {
    #[account(mut)]
    pub author: Signer<'info>,

    #[account(
        init,
        payer = author,
        space = WishAccount::LEN,
        seeds = [
            b"wish",
            author.key().as_ref(),
            &author_wishes.total_wishes.to_le_bytes(),
        ],
        bump,
    )]
    pub wish_account: Account<'info, WishAccount>,

    #[account(
        mut,
        seeds = [b"author_wishes", author.key().as_ref()],
        bump = author_wishes.bump,
    )]
    pub author_wishes: Account<'info, AuthorWishes>,

    pub system_program: Program<'info, System>,
}

// ============ 错误码 ============

#[error_code]
pub enum WishError {
    #[msg("心愿内容不能为空")]
    ContentEmpty,
    #[msg("心愿内容过长，最多200个字节")]
    ContentTooLong,
}