# StateFi Protocol

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

StateFi Protocol is a financial infrastructure built on Solana that enables seamless conversion between fiat currencies, SPL tokens (Solana-based tokens) and NFTs.

## Overview

StateFi Protocol bridges traditional finance and Solana's blockchain ecosystem by providing secure, efficient, and compliant pathways for moving assets between fiat currencies and digital tokens/collectibles.

Currently, the protocol supports two core functionalities:

- **Fiat to SPL Tokens**: Deposit fiat currency and receive equivalent SPL tokens in your wallet
- **SPL Tokens to Fiat**: Convert your SPL tokens back to fiat with easy bank withdrawals

## Architecture

StateFi Protocol is built on the Solana blockchain using the Anchor framework. The protocol consists of the following main components:

- **Protocol Configuration**: Manages protocol-wide settings and admin controls
- **User Profiles**: KYC-ready user management system
- **Vaults**: Secure storage for user assets
- **Token Whitelist**: Security mechanism for supported SPL tokens
- **Deposit/Withdrawal Processing**: Handles the conversion processes

## Features

### Fiat to SPL Token Conversion

Users can deposit fiat currency and receive SPL tokens through a simple process:

1. User initiates a fiat deposit via integrated bank transfer
2. The protocol creates a `FiatDeposit` account with pending status
3. After off-chain verification, the deposit is processed and tokens are minted
4. SPL tokens are deposited into the user's vault
5. Users can withdraw tokens to their Solana wallet

### SPL Token to Fiat Conversion

Users can convert their SPL tokens back to fiat currency:

1. User initiates a withdrawal by specifying amount and token
2. The protocol locks the tokens in a `FiatWithdrawal` account
3. The off-ramp service processes the withdrawal
4. Fiat is sent to the user's bank account

### Security Features

- Token whitelisting ensures only verified assets can be used
- KYC verification system for regulatory compliance
- Admin fee system with configurable rates
- Secure vaults for asset management

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Solana](https://docs.solanalabs.com/cli/install)
- [Anchor](https://www.anchor-lang.com/docs/installation)
- [Node.js](https://nodejs.org/) (for front-end integration)

### Installation

1. Clone the repository
```bash
git clone https://github.com/francis-codex/statefi-protocol.git
cd statefi-protocol
```

2. Build the program
```bash
anchor build
```

3. Deploy to a Solana cluster
```bash
anchor deploy
```

## Usage

### Creating a User Profile

```typescript
const createUserProfile = async (name, email) => {
  const tx = await program.methods
    .createUserProfile(name, email)
    .accounts({
      user: wallet.publicKey,
      userProfile: getUserProfilePDA(wallet.publicKey),
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
  
  console.log("User profile created with tx:", tx);
};
```

### Creating a Vault

```typescript
const createVault = async () => {
  const tx = await program.methods
    .createVault()
    .accounts({
      user: wallet.publicKey,
      userProfile: getUserProfilePDA(wallet.publicKey),
      vault: getVaultPDA(wallet.publicKey),
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
  
  console.log("Vault created with tx:", tx);
};
```

### Initiating a Fiat Deposit

```typescript
const initiateFiatDeposit = async (amount, tokenMint, referenceId) => {
  const tx = await program.methods
    .initiateFiatDeposit(new anchor.BN(amount), referenceId)
    .accounts({
      user: wallet.publicKey,
      userProfile: getUserProfilePDA(wallet.publicKey),
      mint: tokenMint,
      tokenWhitelist: getTokenWhitelistPDA(tokenMint),
      fiatDeposit: getFiatDepositPDA(wallet.publicKey, referenceId),
      userTokenAccount: getUserTokenAccount(wallet.publicKey, tokenMint),
      treasuryTokenAccount: getTreasuryTokenAccount(tokenMint),
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
  
  console.log("Fiat deposit initiated with tx:", tx);
};
```

### Initiating a Fiat Withdrawal

```typescript
const initiateFiatWithdrawal = async (amount, tokenMint, referenceId) => {
  const tx = await program.methods
    .initiateFiatWithdrawal(new anchor.BN(amount), referenceId)
    .accounts({
      user: wallet.publicKey,
      userProfile: getUserProfilePDA(wallet.publicKey),
      vault: getVaultPDA(wallet.publicKey),
      tokenWhitelist: getTokenWhitelistPDA(tokenMint),
      mint: tokenMint,
      vaultTokenAccount: getVaultTokenAccount(getVaultPDA(wallet.publicKey), tokenMint),
      treasuryTokenAccount: getTreasuryTokenAccount(tokenMint),
      fiatWithdrawal: getFiatWithdrawalPDA(wallet.publicKey, tokenMint, referenceId),
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();
  
  console.log("Fiat withdrawal initiated with tx:", tx);
};
```

## Program PDAs and Seeds

The protocol uses the following PDAs (Program Derived Addresses):

- Protocol Config: `["protocol_config"]`
- User Profile: `["user_profile", user_pubkey]`
- Vault: `["vault", user_pubkey]`
- Token Whitelist: `["token_whitelist", mint_pubkey]`
- Fiat Deposit: `["fiat_deposit", user_pubkey, reference_id]`
- Fiat Withdrawal: `["fiat_withdrawal", user_pubkey, mint_pubkey, reference_id]`

## Tests
~~~test
statefi-protocol
Admin balance: 10 SOL
User balance: 10 SOL
    ✔ Initialize protocol (403ms)
    ✔ Create user profile (441ms)
    ✔ Create vault (420ms)
    ✔ Whitelist token (483ms)
    ✔ Initiate and complete fiat deposit (1313ms)
Verified that 10001 basis points is greater than the maximum 10000 (100%)
Verified that actual protocol fee (100 basis points) is <= max
    ✔ Should validate admin fee basis points


  6 passing (4s)

Done in 8.03s.
~~~

## Deployment
~~~test
Signature: 4CtadqL9wHFk6XBBvZsU9ZcAEFhwGHAvCnsez4zGRVreVszLpSMh1ezbhdCnndS8m96VwsPXDTmRki98Y6h84tKY
~~~

## Security Considerations

StateFi Protocol includes several security measures:

- Admin authorization checks
- Token whitelisting
- Deposit/withdrawal status verification
- Input validation
- Secure fund handling

## Future Developments

While we currently focus on fiat-to-SPL and SPL-to-fiat conversions, our roadmap includes:

- SPL to SPL token swaps via Jupiter DEX
- NFT integration via Tensor/Magic Eden
- NFT to SPL token conversions
- SPL token to NFT conversions

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Disclaimer

This protocol is provided as-is. Users should conduct their own research and risk assessment before using the protocol for financial transactions.
