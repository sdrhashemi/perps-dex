import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PerpsDex } from "../target/types/perps_dex";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getMinimumBalanceForRentExemptAccount,
  createInitializeAccountInstruction,
} from "@solana/spl-token";

// Helper: Create PDA-owned token account (not ATA)
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
      space: 165, // token account size
      lamports,
      programId: TOKEN_PROGRAM_ID,
    }),
    createInitializeAccountInstruction(newAccount.publicKey, mint, ownerPda)
  );
  await sendAndConfirmTransaction(connection, tx, [payer, newAccount]);
  return newAccount.publicKey;
}

describe("High-Frequency Trading Simulation", () => {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.PerpsDex as Program<PerpsDex>;

  // PDAs
  let marketPda: PublicKey;
  let marketBump: number;
  let orderbookPda: PublicKey;
  let orderbookBump: number;
  let eqPda: PublicKey;
  let eqBump: number;
  let marginPda: PublicKey;
  let marginBump: number;

  // Token & user
  let mint: PublicKey;
  let marketVault: PublicKey;
  let user: Keypair;
  let userCollateral: PublicKey;

  const marketNonce = 0;

  before(async () => {
    // Mint
    mint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      6
    );

    // Market PDA
    [marketPda, marketBump] = await PublicKey.findProgramAddressSync(
      [
        Buffer.from("market"),
        mint.toBuffer(),
        mint.toBuffer(),
        Buffer.from([marketNonce]),
      ],
      program.programId
    );

    // Vault (token account owned by PDA)
    marketVault = await createPdaOwnedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      mint,
      marketPda
    );

    // User setup
    user = Keypair.generate();
    await provider.connection.requestAirdrop(user.publicKey, 1e9);

    // Margin PDA
    [marginPda, marginBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("margin"), marketPda.toBuffer(), user.publicKey.toBuffer()],
      program.programId
    );

    // Orderbook PDA (side=0 bid)
    [orderbookPda, orderbookBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("orderbook"), marketPda.toBuffer(), Buffer.from([0])], // 0 for bid
      program.programId
    );


    // Event queue PDA
    [eqPda, eqBump] = await PublicKey.findProgramAddressSync(
      [Buffer.from("eventqueue"), marketPda.toBuffer()],
      program.programId
    );

    // User collateral account
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
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    // Initialize margin account
    await program.methods
      .initializeMargin()
      .accounts({
        market: marketPda,
        margin: marginPda,
        user: user.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .signers([user])
      .rpc();

    // Deposit collateral
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

    // Initialize orderbook
    await program.methods
      .initializeOrderbook({ bid: {} })
      .accounts({
        orderbookSide: orderbookPda,
        market: marketPda,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();

    // Initialize event queue
    await program.methods
      .initializeEventQueue()
      .accounts({
        eventQueue: eqPda,
        market: marketPda,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      } as any)
      .rpc();
  });

  it("batches multiple limit orders under CU budget", async () => {
    const ix = [];
    ix.push(
      anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 })
    );
    console.log(program.idl.instructions.find(ix => ix.name === "placeLimitOrder"));

    for (let i = 0; i < 10; i++) {
      ix.push(
        await program.methods
          .placeLimitOrder(
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
            systemProgram: SystemProgram.programId,
          } as any)
          .instruction()
      );
    }

    const tx = new Transaction().add(...ix);
    await provider.sendAndConfirm(tx, [user]);

    console.log("✅ Batching 10 bids stayed under 200k CU");
  });
});
