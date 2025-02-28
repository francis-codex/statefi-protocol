use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use std::mem::size_of;

declare_id!("8pwyvcK1a2MkNnd2M63ec1cz8GH7sKgpVcrMuYCPVYsb");

#[program]
pub mod statefi_protocol{
    use super::*;

    /// Initialize the protocol with admin settings
    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>,
        admin_fee_basis_points: u16,
    ) -> Result<()> {
        require!(
            admin_fee_basis_points <= 10000,
            StateFiError::InvalidFeeBasisPoints
        );

        let protocol_config = &mut ctx.accounts.protocol_config;
        protocol_config.admin = ctx.accounts.admin.key();
        protocol_config.admin_fee_basis_points = admin_fee_basis_points;
        protocol_config.bump = ctx.bumps.protocol_config;

        msg!("Protocol initialized with admin: {}", protocol_config.admin);
        Ok(())
    }

    /// Create a user profile that's required for all operations
    pub fn create_user_profile(
        ctx: Context<CreateUserProfile>,
        name: String,
        email: String,
    ) -> Result<()> {
        require!(name.len() <= 50, StateFiError::StringTooLong);
        require!(email.len() <= 100, StateFiError::StringTooLong);

        let user_profile = &mut ctx.accounts.user_profile;
        user_profile.owner = ctx.accounts.user.key();
        user_profile.name = name;
        user_profile.email = email;
        user_profile.is_kyc_verified = false; // KYC verification happens off-chain
        user_profile.created_at = Clock::get()?.unix_timestamp;
        user_profile.bump = ctx.bumps.user_profile;

        msg!("User profile created for: {}", user_profile.owner);
        Ok(())
    }

    /// Create a vault for a user to store tokens
    pub fn create_vault(ctx: Context<CreateVault>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.owner = ctx.accounts.user_profile.owner;
        vault.created_at = Clock::get()?.unix_timestamp;
        vault.bump = ctx.bumps.vault;

        msg!("Vault created for user: {}", vault.owner);
        Ok(())
    }

    /// Whitelist a new token for use in the protocol
    pub fn whitelist_token(
        ctx: Context<WhitelistToken>,
        symbol: String,
        name: String,
        is_stable: bool,
    ) -> Result<()> {
        require!(symbol.len() <= 10, StateFiError::StringTooLong);
        require!(name.len() <= 50, StateFiError::StringTooLong);

        let token_whitelist = &mut ctx.accounts.token_whitelist;
        token_whitelist.mint = ctx.accounts.mint.key();
        token_whitelist.symbol = symbol;
        token_whitelist.name = name;
        token_whitelist.is_stable = is_stable;
        token_whitelist.is_active = true;
        token_whitelist.created_at = Clock::get()?.unix_timestamp;
        token_whitelist.bump = ctx.bumps.token_whitelist;

        msg!("Token whitelisted: {}", token_whitelist.mint);
        Ok(())
    }

    /// Initiate a fiat deposit which will be processed by an off-chain service
    pub fn initiate_fiat_deposit(
        ctx: Context<InitiateFiatDeposit>,
        amount: u64,
        reference_id: String,
    ) -> Result<()> {
        require!(amount > 0, StateFiError::InvalidAmount);
        require!(reference_id.len() <= 100, StateFiError::StringTooLong);

        let fiat_deposit = &mut ctx.accounts.fiat_deposit;
        fiat_deposit.user = ctx.accounts.user_profile.owner;
        fiat_deposit.mint = ctx.accounts.mint.key();
        fiat_deposit.amount = amount;
        fiat_deposit.reference_id = reference_id;
        fiat_deposit.status = DepositStatus::Pending;
        fiat_deposit.created_at = Clock::get()?.unix_timestamp;
        fiat_deposit.updated_at = fiat_deposit.created_at;
        fiat_deposit.bump = ctx.bumps.fiat_deposit;

        msg!("Fiat deposit initiated for user: {} with amount: {}", fiat_deposit.user, amount);
        Ok(())
    }

    /// Complete a fiat deposit (called by admin after off-chain verification)
    pub fn complete_fiat_deposit(ctx: Context<CompleteFiatDeposit>) -> Result<()> {
        let fiat_deposit = &mut ctx.accounts.fiat_deposit;
        let vault = &ctx.accounts.vault;
        let protocol_config = &ctx.accounts.protocol_config;

        // Ensure deposit is still pending
        require!(
            fiat_deposit.status == DepositStatus::Pending,
            StateFiError::InvalidDepositStatus
        );

        // Ensure vault belongs to the user who initiated the deposit
        require!(
            vault.owner == fiat_deposit.user,
            StateFiError::InvalidVaultOwner
        );

        // Calculate fees if any
        let fee_amount = if protocol_config.admin_fee_basis_points > 0 {
            (fiat_deposit.amount as u128)
                .checked_mul(protocol_config.admin_fee_basis_points as u128)
                .unwrap()
                .checked_div(10000)
                .unwrap() as u64
        } else {
            0
        };

        let user_amount = fiat_deposit.amount.checked_sub(fee_amount).unwrap();

        // Mint tokens to user's vault token account
        let seeds = &[
            b"protocol_config".as_ref(),
            &[protocol_config.bump],
        ];
        let signer = &[&seeds[..]];

        // Transfer tokens to user
        let cpi_accounts = Transfer {
            from: ctx.accounts.treasury_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.protocol_config.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, user_amount)?;

        // If there's a fee, transfer to the admin account
        if fee_amount > 0 {
            let fee_cpi_accounts = Transfer {
                from: ctx.accounts.treasury_token_account.to_account_info(),
                to: ctx.accounts.admin_token_account.to_account_info(),
                authority: ctx.accounts.protocol_config.to_account_info(),
            };
            let fee_cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                fee_cpi_accounts,
                signer,
            );
            token::transfer(fee_cpi_ctx, fee_amount)?;
        }

        // Update deposit status
        fiat_deposit.status = DepositStatus::Completed;
        fiat_deposit.updated_at = Clock::get()?.unix_timestamp;

        msg!("Fiat deposit completed for user: {} with amount: {}", fiat_deposit.user, user_amount);
        Ok(())
    }

    /// Initiate withdrawal of SPL tokens to fiat
    pub fn initiate_fiat_withdrawal(
        ctx: Context<InitiateFiatWithdrawal>,
        amount: u64,
        reference_id: String,
    ) -> Result<()> {
        require!(amount > 0, StateFiError::InvalidAmount);
        require!(reference_id.len() <= 100, StateFiError::StringTooLong);

        // Transfer tokens from user's vault to protocol treasury
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_token_account.to_account_info(),
            to: ctx.accounts.treasury_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Create withdrawal record
        let fiat_withdrawal = &mut ctx.accounts.fiat_withdrawal;
        fiat_withdrawal.user = ctx.accounts.user_profile.owner;
        fiat_withdrawal.mint = ctx.accounts.mint.key();
        fiat_withdrawal.amount = amount;
        fiat_withdrawal.reference_id = reference_id;
        fiat_withdrawal.status = WithdrawalStatus::Pending;
        fiat_withdrawal.created_at = Clock::get()?.unix_timestamp;
        fiat_withdrawal.updated_at = fiat_withdrawal.created_at;
        fiat_withdrawal.bump = ctx.bumps.fiat_withdrawal;

        msg!("Fiat withdrawal initiated for user: {} with amount: {}", fiat_withdrawal.user, amount);
        Ok(())
    }

    /// Complete a fiat withdrawal (called by admin after off-chain processing)
    pub fn complete_fiat_withdrawal(ctx: Context<CompleteFiatWithdrawal>) -> Result<()> {
        let fiat_withdrawal = &mut ctx.accounts.fiat_withdrawal;

        // Ensure withdrawal is still pending
        require!(
            fiat_withdrawal.status == WithdrawalStatus::Pending,
            StateFiError::InvalidWithdrawalStatus
        );

        // Update withdrawal status
        fiat_withdrawal.status = WithdrawalStatus::Completed;
        fiat_withdrawal.updated_at = Clock::get()?.unix_timestamp;

        msg!("Fiat withdrawal completed for user: {}", fiat_withdrawal.user);
        Ok(())
    }

    /// Cancel a pending fiat withdrawal and return tokens to user
    pub fn cancel_fiat_withdrawal(ctx: Context<CancelFiatWithdrawal>) -> Result<()> {
        let fiat_withdrawal = &mut ctx.accounts.fiat_withdrawal;
        let protocol_config = &ctx.accounts.protocol_config;

        // Ensure withdrawal is still pending
        require!(
            fiat_withdrawal.status == WithdrawalStatus::Pending,
            StateFiError::InvalidWithdrawalStatus
        );

        // Return tokens from treasury to user's vault
        let seeds = &[
            b"protocol_config".as_ref(),
            &[protocol_config.bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.treasury_token_account.to_account_info(),
            to: ctx.accounts.vault_token_account.to_account_info(),
            authority: ctx.accounts.protocol_config.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, fiat_withdrawal.amount)?;

        // Update withdrawal status
        fiat_withdrawal.status = WithdrawalStatus::Cancelled;
        fiat_withdrawal.updated_at = Clock::get()?.unix_timestamp;

        msg!("Fiat withdrawal cancelled for user: {}", fiat_withdrawal.user);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeProtocol<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        init,
        payer = admin,
        space = 8 + size_of::<ProtocolConfig>(),
        seeds = [b"protocol_config"],
        bump
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateUserProfile<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        payer = user,
        space = 8 + size_of::<UserProfile>() + 50 + 100, // Extra space for name and email
        seeds = [b"user_profile", user.key().as_ref()],
        bump
    )]
    pub user_profile: Account<'info, UserProfile>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateVault<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        constraint = user.key() == user_profile.owner @ StateFiError::InvalidOwner,
        seeds = [b"user_profile", user.key().as_ref()],
        bump = user_profile.bump,
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        init,
        payer = user,
        space = 8 + size_of::<Vault>(),
        seeds = [b"vault", user_profile.owner.as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WhitelistToken<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        has_one = admin @ StateFiError::Unauthorized,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = admin,
        space = 8 + size_of::<TokenWhitelist>() + 10 + 50, // Extra space for symbol and name
        seeds = [b"token_whitelist", mint.key().as_ref()],
        bump
    )]
    pub token_whitelist: Account<'info, TokenWhitelist>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(amount: u64, reference_id: String)]
pub struct InitiateFiatDeposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_profile: Account<'info, UserProfile>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub token_whitelist: Account<'info, TokenWhitelist>,
    #[account(
        init,
        payer = user,
        space = 8 + std::mem::size_of::<FiatDeposit>(),
        seeds = [b"fiat_deposit", user.key().as_ref(), reference_id.as_bytes()],
        bump
    )]
    pub fiat_deposit: Account<'info, FiatDeposit>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub treasury_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CompleteFiatDeposit<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        has_one = admin @ StateFiError::Unauthorized,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    #[account(mut)]
    pub fiat_deposit: Account<'info, FiatDeposit>,

    #[account(
        seeds = [b"vault", fiat_deposit.user.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        constraint = vault_token_account.owner == vault.key() @ StateFiError::InvalidTokenAccountOwner,
        constraint = vault_token_account.mint == fiat_deposit.mint @ StateFiError::InvalidMint,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_token_account.mint == fiat_deposit.mint @ StateFiError::InvalidMint,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = admin_token_account.mint == fiat_deposit.mint @ StateFiError::InvalidMint,
        constraint = admin_token_account.owner == protocol_config.admin @ StateFiError::InvalidTokenAccountOwner,
    )]
    pub admin_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
#[instruction(amount: u64, reference_id: String)]
pub struct InitiateFiatWithdrawal<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [b"user_profile", user.key().as_ref()],
        bump = user_profile.bump,
    )]
    pub user_profile: Account<'info, UserProfile>,

    #[account(
        seeds = [b"vault", user_profile.owner.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        seeds = [b"token_whitelist", mint.key().as_ref()],
        bump = token_whitelist.bump,
        constraint = token_whitelist.is_active @ StateFiError::TokenNotActive,
    )]
    pub token_whitelist: Account<'info, TokenWhitelist>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = vault_token_account.owner == vault.key() @ StateFiError::InvalidTokenAccountOwner,
        constraint = vault_token_account.mint == mint.key() @ StateFiError::InvalidMint,
        constraint = vault_token_account.amount >= amount @ StateFiError::InsufficientFunds,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_token_account.mint == mint.key() @ StateFiError::InvalidMint,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = user,
        space = 8 + size_of::<FiatWithdrawal>() + 100, // Extra space for reference_id
        seeds = [
            b"fiat_withdrawal",
            user.key().as_ref(),
            mint.key().as_ref(),
            reference_id.as_bytes()
        ],
        bump
    )]
    pub fiat_withdrawal: Account<'info, FiatWithdrawal>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CompleteFiatWithdrawal<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        has_one = admin @ StateFiError::Unauthorized,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    #[account(mut)]
    pub fiat_withdrawal: Account<'info, FiatWithdrawal>,
}

#[derive(Accounts)]
pub struct CancelFiatWithdrawal<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [b"protocol_config"],
        bump = protocol_config.bump,
        has_one = admin @ StateFiError::Unauthorized,
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,

    #[account(mut)]
    pub fiat_withdrawal: Account<'info, FiatWithdrawal>,

    #[account(
        seeds = [b"vault", fiat_withdrawal.user.as_ref()],
        bump = vault.bump,
    )]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        constraint = vault_token_account.owner == vault.key() @ StateFiError::InvalidTokenAccountOwner,
        constraint = vault_token_account.mint == fiat_withdrawal.mint @ StateFiError::InvalidMint,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_token_account.mint == fiat_withdrawal.mint @ StateFiError::InvalidMint,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct ProtocolConfig {
    pub admin: Pubkey,
    pub admin_fee_basis_points: u16, // In basis points (1/100 of a percent, e.g., 10000 = 100%)
    pub bump: u8,
}

#[account]
pub struct UserProfile {
    pub owner: Pubkey,
    pub name: String,
    pub email: String,
    pub is_kyc_verified: bool,
    pub created_at: i64,
    pub bump: u8,
}

#[account]
pub struct Vault {
    pub owner: Pubkey,
    pub created_at: i64,
    pub bump: u8,
}

#[account]
pub struct TokenWhitelist {
    pub mint: Pubkey,
    pub symbol: String,
    pub name: String,
    pub is_stable: bool,
    pub is_active: bool,
    pub created_at: i64,
    pub bump: u8,
}

#[account]
pub struct FiatDeposit {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub reference_id: String,
    pub status: DepositStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[account]
pub struct FiatWithdrawal {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub reference_id: String,
    pub status: WithdrawalStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Debug)]
pub enum DepositStatus {
    Pending,
    Completed,
    Rejected,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Debug)]
pub enum WithdrawalStatus {
    Pending,
    Completed,
    Cancelled,
}

#[error_code]
pub enum StateFiError {
    #[msg("Invalid fee basis points (must be <= 10000)")]
    InvalidFeeBasisPoints,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Invalid deposit status")]
    InvalidDepositStatus,
    #[msg("Invalid withdrawal status")]
    InvalidWithdrawalStatus,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("String too long")]
    StringTooLong,
    #[msg("Invalid vault owner")]
    InvalidVaultOwner,
    #[msg("Invalid token account owner")]
    InvalidTokenAccountOwner,
    #[msg("Invalid mint")]
    InvalidMint,
    #[msg("Token not active")]
    TokenNotActive,
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Invalid owner")]
    InvalidOwner,
}