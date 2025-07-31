import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PerpsDex } from "../target/types/perps_dex";
import { expect } from "chai";

import { PublicKey, Keypair, SystemProgram, Transaction, sendAndConfirmTransaction } from "@solana/web3.js";
import { createMint, getMint, TOKEN_PROGRAM_ID, getMinimumBalanceForRentExemptAccount, createInitializeAccountInstruction, createAccount, mintTo, getAccount } from "@solana/spl-token";

/*
describe("High-Frequency Trading Simulation - Setup", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  it("Initializes Anchor provider and loads program", async () => {
    expect(provider).to.not.be.undefined;
    expect(provider.connection).to.not.be.undefined;
    expect(program).to.not.be.undefined;
    expect(program.programId).to.be.instanceOf(anchor.web3.PublicKey);

    const balance = await provider.connection.getBalance(provider.wallet.publicKey);
    expect(balance).to.be.greaterThan(0, "Provider wallet should have SOL");
  })
})

describe("High-Frequency Trading Simulation - Mint", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  let mint: PublicKey;

  before(async () => {
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      6
    );
  });

  it("Creates and verifies token mint", async () => {
    expect(mint).to.be.instanceOf(PublicKey);

    const mintInfo = await getMint(provider.connection, mint);
    expect(mintInfo.decimals).to.equal(6, "Mint should have 6 decimals");
    expect(mintInfo.mintAuthority.toBase58()).to.equal(provider.wallet.publicKey.toBase58(), "Mint authority should be provider wallet");
    expect(mintInfo.supply.toString()).to.equal("0", "Mint supply should be 0");
  });

});
*/
async function createPdaOwnedTokenAccount(
  connection: anchor.web3.Connection,
  payer: Keypair,
  mint: PublicKey,
  ownerPda: PublicKey
): Promise<PublicKey> {
  const newAccount = Keypair.generate();
  const lamports = await getMinimumBalanceForRentExemptAccount(connection);
  const tx = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: payer.publicKey,
      newAccountPubkey: newAccount.publicKey,
      space: 165,
      lamports,
      programId: TOKEN_PROGRAM_ID
    }),
    createInitializeAccountInstruction(newAccount.publicKey, mint, ownerPda)
  );
  await sendAndConfirmTransaction(connection, tx, [payer, newAccount]);
  return newAccount.publicKey;
}
/*
describe("High-Frequency Trading Simulation - PDAs and Vault", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  let mint: PublicKey;
  let marketPda: PublicKey;
  let marketBump: number;
  let orderbookPda: PublicKey;
  let orderbookBump: number;
  let eqPda: PublicKey;
  let eqBump: number;
  let marginPda: PublicKey;
  let marginBump: number;
  let marketVault: PublicKey;
  const marketNonce = 0;

  before(async () => {
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      6
    );

    [marketPda, marketBump] = await PublicKey.findProgramAddressSync(
      [
        Buffer.from("market"),
        mint.toBuffer(),
        mint.toBuffer(),
        Buffer.from([marketNonce]),
      ],
      program.programId
    );

    marketVault = await createPdaOwnedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      marketPda
    );

    [orderbookPda, orderbookBump] = await PublicKey.findProgramAddressSync(
      [
        Buffer.from("orderbook"),
        marketPda.toBuffer(),
        Buffer.from([0])
      ],
      program.programId
    );

    [eqPda, eqBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("eventqueue"), marketPda.toBuffer()],
      program.programId
    );

    [marginPda, marginBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("margin"), marketPda.toBuffer(), provider.wallet.publicKey.toBuffer()],
      program.programId
    );
  });
  it("Creates PDAs and market vault", async () => {
    expect(marketPda).to.be.instanceOf(PublicKey);
    expect(marketBump).to.be.a("number");
    expect(orderbookPda).to.be.instanceOf(PublicKey);
    expect(orderbookBump).to.be.a("number");
    expect(eqPda).to.be.instanceOf(PublicKey);
    expect(eqBump).to.be.a("number");
    expect(marginPda).to.be.instanceOf(PublicKey);
    expect(marginBump).to.be.a("number");
    expect(marketVault).to.be.instanceOf(PublicKey);

    const vaultInfo = await provider.connection.getAccountInfo(marketVault);
    expect(vaultInfo).to.not.be.null;
    expect(vaultInfo.owner.toBase58()).to.equal(TOKEN_PROGRAM_ID.toBase58());
  });
});

describe("High-Frequency Trading Simulation - User Setup", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  let mint: PublicKey;
  let user: Keypair;
  let userCollateral: PublicKey;

  before(async () => {
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      6
    );

    user = Keypair.generate();
    await provider.connection.requestAirdrop(user.publicKey, 1e9);

    userCollateral = await createAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      user.publicKey
    );
    await mintTo(
      provider.connection,
      provider.wallet.payer,
      mint,
      userCollateral,
      provider.wallet.publicKey,
      1_000_000
    );
  });

  it("Sets up user and collateral account", async () => {
    const userBalance = await provider.connection.getBalance(user.publicKey);
    expect(userBalance).to.be.greaterThan(0, "User should have SOL");

    const accountInfo = await getAccount(provider.connection, userCollateral);
    expect(accountInfo.mint.toBase58()).to.equal(mint.toBase58());
    expect(accountInfo.owner.toBase58()).to.equal(user.publicKey.toBase58());
    expect(accountInfo.amount.toString()).to.equal("1000000", "Collateral should be 1,000,000");
  });
});

describe("High-Frequency Tradin Simulation - Market Initialization", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  let mint: PublicKey;
  let marketPda: PublicKey;
  let marketBump: number;
  const marketNonce = 0;

  before(async () => {
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      6
    );
    [marketPda, marketBump] = await PublicKey.findProgramAddressSync(
      [
        Buffer.from("market"),
        mint.toBuffer(),
        mint.toBuffer(),
        Buffer.from([marketNonce]),
      ],
      program.programId
    );

    await program.methods
      .initializeMarket(marketNonce, {
        tickSize: new anchor.BN(1),
        lotSize: new anchor.BN(1),
        leverageLimit: 20,
        fundingInterval: new anchor.BN(3600),
        maintenanceMarginRatio: 500,
      })
      .accounts({
        market: marketPda,
        baseMint: mint,
        quoteMint: mint,
        authority: provider.wallet.publicKey,
      })
      .rpc();
  });

  it("Initializes market", async () => {
    const marketAccount = await program.account.market.fetch(marketPda);
    expect(marketAccount.baseMint.toBase58()).to.equal(mint.toBase58());
    expect(marketAccount.quoteMint.toBase58()).to.equal(mint.toBase58());
    expect(marketAccount.params.tickSize.toNumber()).to.equal(1);
    expect(marketAccount.params.lotSize.toNumber()).to.equal(1);
    expect(marketAccount.params.leverageLimit).to.equal(20);
    expect(marketAccount.params.maintenanceMarginRatio).to.equal(500);
  });
});
*/
describe("High Frequency Trading Simulation - Account Initialization and Collateral Deposit", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  let mint: PublicKey;
  let marketPda: PublicKey;
  let marketBump: number;
  let orderbookPda: PublicKey;
  let orderbookBump: number;
  let eqPda: PublicKey;
  let eqBump: number;
  let marginPda: PublicKey;
  let marginBump: number;
  let marketVault: PublicKey;
  let user: Keypair;
  let userCollateral: PublicKey;
  const marketNonce = 0;

  before(async () => {
    // Create mint
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      6
    );

    // Set up user
    user = Keypair.generate();
    await provider.connection.requestAirdrop(user.publicKey, 1e9);
    await new Promise((resolve) => setTimeout(resolve, 1000)); // Wait for airdrop to process

    userCollateral = await createAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      user.publicKey
    );
    await mintTo(
      provider.connection,
      provider.wallet.payer,
      mint,
      userCollateral,
      provider.wallet.publicKey,
      1_000_000
    );

    // Set up PDAs
    [marketPda, marketBump] = await PublicKey.findProgramAddressSync(
      [
        Buffer.from("market"),
        mint.toBuffer(),
        mint.toBuffer(),
        Buffer.from([marketNonce]),
      ],
      program.programId
    );

    marketVault = await createPdaOwnedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      marketPda
    );

    [orderbookPda, orderbookBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("orderbook"), marketPda.toBuffer(), Buffer.from([0])],
      program.programId
    );

    [eqPda, eqBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("eventqueue"), marketPda.toBuffer()],
      program.programId
    );

    [marginPda, marginBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("margin"), marketPda.toBuffer(), user.publicKey.toBuffer()],
      program.programId
    );

    // Initialize market
    await program.methods
      .initializeMarket(marketNonce, {
        tickSize: new anchor.BN(1),
        lotSize: new anchor.BN(1),
        leverageLimit: 20,
        fundingInterval: new anchor.BN(3600),
        maintenanceMarginRatio: 500,
      })
      .accounts({
        market: marketPda,
        baseMint: mint,
        quoteMint: mint,
        authority: provider.wallet.publicKey,

      })
      .rpc();

    await program.methods
      .initializeMargin()
      .accounts({
        market: marketPda,
        margin: marginPda,
        user: user.publicKey,

      } as any)
      .signers([user])
      .rpc();

    await program.methods
      .initializeOrderbook({ bid: {} })
      .accounts({
        orderbookSide: orderbookPda,
        market: marketPda,
        authority: provider.wallet.publicKey,

      })
      .rpc();

    // Initialize event queue
    await program.methods
      .initializeEventQueue()
      .accounts({
        eventQueue: eqPda,
        market: marketPda,
        authority: provider.wallet.publicKey,

      } as any)
      .rpc();


  });

  it("Initializes margin, orderbook and event queue", async () => {
    const marginAccount = await program.account.marginAccount.fetch(marginPda);
    expect(marginAccount.owner.toBase58()).to.equal(user.publicKey.toBase58(), "Margin account owner should match user keypair");
    expect(marginAccount.collateral.toString()).to.equal("0", "Margin account collateral should be 0");
    expect(marginAccount.bump).to.equal(marginBump, "Margin account bump should match");
    const orderbookAccount = await program.account.orderbookSide.fetch(orderbookPda);
    expect(orderbookAccount.market.toBase58()).to.equal(marketPda.toBase58(), "Orderbook market should match market PDA");

    const eventQueueAccount = await program.account.eventQueue.fetch(eqPda);
    expect(eventQueueAccount.market.toBase58()).to.equal(marketPda.toBase58(), "Event queue market should match market PDA");
  });
  it("Deposits collateral into margin account", async () => {
    await program.methods
      .depositCollateral(new anchor.BN(1_000_000))
      .accounts({
        market: marketPda,
        authority: provider.wallet.publicKey,
        margin: marginPda,
        user: user.publicKey,
        userCollateral: userCollateral,
        marketVault: marketVault,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .signers([user])
      .rpc();

    // Verify market vault
    const vaultInfo = await getAccount(provider.connection, marketVault);
    expect(vaultInfo.amount.toString()).to.equal("1000000", "Market vault should have 1,000,000 tokens");

    // Verify user collateral
    const userCollateralInfo = await getAccount(provider.connection, userCollateral);
    expect(userCollateralInfo.amount.toString()).to.equal("0", "User collateral should be empty after deposit");

    // Verify margin account
    const marginAccount = await program.account.marginAccount.fetch(marginPda);
    expect(marginAccount.collateral.toString()).to.equal("1000000", "Margin account should reflect deposited collateral");
  });
  it("Batches multiple limit orders under CU budget", async () => {
    const ix = [];
    ix.push(
      anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 })
    );

    for (let i = 0; i < 10; i++) {
      ix.push(
        await program.methods
          .placeLimitOrder(
            { bid: {} },
            new anchor.BN(1000 + i),
            new anchor.BN(10)
          )
          .accounts({
            orderbookSide: orderbookPda,
            eventQueue: eqPda,
            margin: marginPda,
            user: user.publicKey,
            market: marketPda,
            tokenProgram: TOKEN_PROGRAM_ID,
          } as any)
          .signers([user])
          .instruction()
      );
    }

    const tx = new Transaction().add(...ix);
    const txSig = await provider.sendAndConfirm(tx, [user]);
    console.log("Transaction signature:", txSig);

    // Verify orders in orderbook
    const orderbookAccount = await program.account.orderbookSide.fetch(orderbookPda);
    expect(orderbookAccount.slab.length).to.be.greaterThanOrEqual(10, "Orderbook should contain at least 10 orders");
  })
});

/*
describe("High Frequency Trading Simulation - Collateral Deposit", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  let mint: PublicKey;
  let marketPda: PublicKey;
  let marketBump: number;
  let marginPda: PublicKey;
  let marginBump: number;
  let marketVault: PublicKey;
  let user: Keypair;
  let userCollateral: PublicKey;
  const marketNonce = 0;

  before(async () => {
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      6
    );

    [marketPda, marketBump] = await PublicKey.findProgramAddressSync(
      [
        Buffer.from("market"),
        mint.toBuffer(),
        mint.toBuffer(),
        Buffer.from([marketNonce]),
      ],
      program.programId
    );

    marketVault = await createPdaOwnedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      marketPda
    );

    [marginPda, marginBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("margin"), marketPda.toBuffer(), provider.wallet.publicKey.toBuffer()],
      program.programId
    );

    user = Keypair.generate();
    await provider.connection.requestAirdrop(user.publicKey, 1e9);

    userCollateral = await createAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      user.publicKey
    );
    await mintTo(
      provider.connection,
      provider.wallet.payer,
      mint,
      userCollateral,
      provider.wallet.publicKey,
      1_000_000
    );

    await program.methods
      .initializeMarket(marketNonce, {
        tickSize: new anchor.BN(1),
        lotSize: new anchor.BN(1),
        leverageLimit: 20,
        fundingInterval: new anchor.BN(3600),
        maintenanceMarginRatio: 500,
      })
      .accounts({
        market: marketPda,
        baseMint: mint,
        quoteMint: mint,
        authority: provider.wallet.publicKey,
      })
      .rpc();

    await program.methods
      .initializeMargin()
      .accounts({
        market: marketPda,
        margin: marginPda,
        user: user.publicKey,
      } as any)
      .signers([user])
      .rpc();

    await program.methods
      .depositCollateral(new anchor.BN(1_000_000))
      .accounts({
        market: marketPda,
        authority: provider.wallet.publicKey,
        margin: marginPda,
        user: user.publicKey,
        userCollateral: userCollateral,
        marketVault: marketVault,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .signers([user])
      .rpc()
  });

  it("Deposits collateral into margin account", async () => {
    const vaultInfo = await getAccount(provider.connection, marketVault);
    expect(vaultInfo.amount.toString()).to.equal("1000000", "Market vault should have 1,000,000 tokens");

    const userCollateralInfo = await getAccount(provider.connection, userCollateral);
    expect(userCollateralInfo.amount.toString()).to.equal("0", "User collateral should be 0");

    const marginAccount = await program.account.marginAccount.fetch(marginPda);
    expect(marginAccount.collateral.toNumber()).to.equal(1_000_000, "Margin account should reflect deposited collateral");

  })
})
*/