// Everything runs in LiteSVM so the clock can be set. Rewards are per day and the oracle
// cares what hour it is, so a real validator would make these tests depend on when you run them.
import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { NftStaking } from "../target/types/nft_staking";
import {
  ComputeBudgetProgram,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  AccountLayout,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import {
  CheckResult,
  deserializeAssetV1,
  deserializeCollectionV1,
} from "@metaplex-foundation/mpl-core";
import { publicKey as umiKey, lamports } from "@metaplex-foundation/umi";
import { LiteSVM, FailedTransactionMetadata } from "litesvm";
import { BN } from "bn.js";
import { expect } from "chai";

const CORE = new PublicKey("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
const ONE = 1_000_000; // reward mint has 6 decimals
const DAY = 86_400;
const HOUR = 3_600;
const POINTS_PER_DAY = 100 * ONE;
const BURN_BONUS = 5_000 * ONE;
const FREEZE_PERIOD = 7 * DAY;
const CRANK_REWARD = 1_000_000;
// core's "rejected" error. after hours the asset isnt frozen, so only the oracle can be saying no
const CORE_REJECTED = "custom program error: 0x9";
const TX_FEE = 5_000;

// 2026-10-05 00:00:00 UTC. midnight makes the hour math easy to read
const DAY0 = 1_791_158_400;

describe("core nft staking", () => {
  const program = anchor.workspace.nftStaking as Program<NftStaking>;
  const pda = (...seeds: Buffer[]) =>
    PublicKey.findProgramAddressSync(seeds, program.programId)[0];

  let svm: LiteSVM;
  const admin = Keypair.generate();
  const alice = Keypair.generate();
  const bob = Keypair.generate();
  const cranker = Keypair.generate();
  const collection = Keypair.generate();

  const config = pda(Buffer.from("config"));
  const rewardMint = pda(Buffer.from("rewards"), config.toBuffer());
  const oracle = pda(Buffer.from("oracle"));
  const vault = pda(Buffer.from("vault"));
  const updateAuthority = pda(Buffer.from("update_authority"), collection.publicKey.toBuffer());
  const stakePda = (asset: PublicKey) => pda(Buffer.from("stake"), asset.toBuffer());
  const rewardAta = (owner: PublicKey) => getAssociatedTokenAddressSync(rewardMint, owner);

  const send = (ixs: TransactionInstruction[], payer: Keypair, extra: Keypair[] = []) => {
    // new blockhash every time, otherwise the same crank twice looks like a duplicate tx
    svm.expireBlockhash();
    const tx = new Transaction();
    tx.recentBlockhash = svm.latestBlockhash();
    tx.feePayer = payer.publicKey;
    // core CPIs are hungry, stake does three of them
    tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 1_400_000 }));
    ixs.forEach((ix) => tx.add(ix));
    tx.sign(payer, ...extra);
    return svm.sendTransaction(tx);
  };

  const sendOk = (ixs: TransactionInstruction[], payer: Keypair, extra: Keypair[] = []) => {
    const res = send(ixs, payer, extra);
    if (res instanceof FailedTransactionMetadata) {
      console.log(res.meta().logs());
      throw new Error("tx failed: " + res.err().toString());
    }
  };

  const expectFail = (
    ixs: TransactionInstruction[],
    payer: Keypair,
    needle: string,
    extra: Keypair[] = []
  ) => {
    const res = send(ixs, payer, extra);
    expect(res).to.be.instanceOf(FailedTransactionMetadata);
    const failed = res as FailedTransactionMetadata;
    const everything = failed.err().toString() + "\n" + failed.meta().logs().join("\n");
    expect(everything).to.include(needle);
  };

  const warpTo = (unixTimestamp: number) => {
    const clock = svm.getClock();
    clock.unixTimestamp = BigInt(unixTimestamp);
    svm.setClock(clock);
  };

  // umi's deserializers want an RpcAccount, so dress the raw bytes up as one
  const rpcAccount = (address: PublicKey) => {
    const acc = svm.getAccount(address);
    return {
      publicKey: umiKey(address.toBase58()),
      data: Uint8Array.from(acc.data),
      executable: false,
      owner: umiKey(acc.owner.toBase58()),
      lamports: lamports(acc.lamports),
    };
  };
  const readAsset = (asset: PublicKey) => deserializeAssetV1(rpcAccount(asset));
  const totalStaked = () => {
    const c = deserializeCollectionV1(rpcAccount(collection.publicKey));
    return c.attributes.attributeList.find((a) => a.key === "total_staked").value;
  };
  const rewardBalance = (owner: PublicKey) => {
    const acc = svm.getAccount(rewardAta(owner));
    return acc ? Number(AccountLayout.decode(acc.data).amount) : 0;
  };
  const oracleTransferState = () =>
    program.coder.accounts.decode("oracle", Buffer.from(svm.getAccount(oracle).data)).transfer;

  const mintAsset = async (owner: Keypair, name: string) => {
    const asset = Keypair.generate();
    const ix = await program.methods
      .mintAsset(name, "https://example.com/nft.json")
      .accountsStrict({
        user: owner.publicKey,
        config,
        asset: asset.publicKey,
        collection: collection.publicKey,
        updateAuthority,
        coreProgram: CORE,
        systemProgram: SystemProgram.programId,
      })
      .instruction();
    sendOk([ix], owner, [asset]);
    return asset.publicKey;
  };

  const stakeIx = (owner: Keypair, asset: PublicKey) =>
    program.methods
      .stake()
      .accountsStrict({
        owner: owner.publicKey,
        config,
        stakeAccount: stakePda(asset),
        asset,
        collection: collection.publicKey,
        updateAuthority,
        coreProgram: CORE,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

  const claimIx = (owner: Keypair, asset: PublicKey) =>
    program.methods
      .claimRewards()
      .accountsStrict({
        owner: owner.publicKey,
        config,
        stakeAccount: stakePda(asset),
        rewardMint,
        ownerRewardAta: rewardAta(owner.publicKey),
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

  // unstake and burn take the exact same accounts
  const exitAccounts = (owner: Keypair, asset: PublicKey) => ({
    owner: owner.publicKey,
    config,
    stakeAccount: stakePda(asset),
    asset,
    collection: collection.publicKey,
    updateAuthority,
    rewardMint,
    ownerRewardAta: rewardAta(owner.publicKey),
    coreProgram: CORE,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  });
  const unstakeIx = (owner: Keypair, asset: PublicKey) =>
    program.methods.unstake().accountsStrict(exitAccounts(owner, asset)).instruction();
  const burnIx = (owner: Keypair, asset: PublicKey) =>
    program.methods.burnStakedNft().accountsStrict(exitAccounts(owner, asset)).instruction();

  const crankIx = () =>
    program.methods
      .crankOracle()
      .accountsStrict({
        cranker: cranker.publicKey,
        oracle,
        vault,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

  const transferIx = (from: Keypair, to: PublicKey, asset: PublicKey) =>
    program.methods
      .transferAsset()
      .accountsStrict({
        owner: from.publicKey,
        newOwner: to,
        config,
        asset,
        collection: collection.publicKey,
        oracle,
        coreProgram: CORE,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

  // crank and return how many lamports the cranker made off it (fee already taken out)
  const crankAt = async (unixTimestamp: number) => {
    warpTo(unixTimestamp);
    const before = Number(svm.getBalance(cranker.publicKey));
    sendOk([await crankIx()], cranker);
    return Number(svm.getBalance(cranker.publicKey)) - before + TX_FEE;
  };

  before(async () => {
    svm = new LiteSVM();
    svm.addProgramFromFile(program.programId, "target/deploy/nft_staking.so");
    svm.addProgramFromFile(CORE, "tests/fixtures/mpl_core.so");
    for (const kp of [admin, alice, bob, cranker]) {
      svm.airdrop(kp.publicKey, BigInt(100 * LAMPORTS_PER_SOL));
    }

    // 10am on day 0, so the oracle starts out open
    warpTo(DAY0 + 10 * HOUR);

    sendOk(
      [
        await program.methods
          .initializeConfig(new BN(POINTS_PER_DAY), new BN(BURN_BONUS), new BN(FREEZE_PERIOD))
          .accountsStrict({
            admin: admin.publicKey,
            config,
            rewardMint,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
          })
          .instruction(),
        await program.methods
          .initOracle(new BN(LAMPORTS_PER_SOL))
          .accountsStrict({
            admin: admin.publicKey,
            config,
            oracle,
            vault,
            systemProgram: SystemProgram.programId,
          })
          .instruction(),
        await program.methods
          .createCollection("Turbin3 Stakers", "https://example.com/collection.json")
          .accountsStrict({
            admin: admin.publicKey,
            config,
            collection: collection.publicKey,
            updateAuthority,
            oracle,
            coreProgram: CORE,
            systemProgram: SystemProgram.programId,
          })
          .instruction(),
      ],
      admin,
      [collection]
    );
  });

  describe("task 1: staking with core plugins", () => {
    let first: PublicKey;
    let third: PublicKey;
    let stakedAt: number;

    it("collection starts with total_staked = 0 and the oracle hooked up", () => {
      expect(totalStaked()).to.equal("0");
      const c = deserializeCollectionV1(rpcAccount(collection.publicKey));
      expect(c.oracles).to.have.length(1);
      expect(c.oracles[0].baseAddress.toString()).to.equal(oracle.toBase58());
      expect(c.oracles[0].lifecycleChecks.transfer).to.deep.equal([CheckResult.CAN_REJECT]);
    });

    it("stake freezes the NFT, adds the burn delegate and bumps total_staked", async () => {
      first = await mintAsset(alice, "Staker #1");
      stakedAt = DAY0 + 10 * HOUR;
      sendOk([await stakeIx(alice, first)], alice);

      const asset = readAsset(first);
      expect(asset.freezeDelegate.frozen).to.equal(true);
      expect(asset.freezeDelegate.authority.address.toString()).to.equal(updateAuthority.toBase58());
      expect(asset.burnDelegate.authority.address.toString()).to.equal(updateAuthority.toBase58());
      expect(totalStaked()).to.equal("1");
    });

    it("a staked NFT cant be transferred, even during market hours", async () => {
      expectFail([await transferIx(alice, bob.publicKey, first)], alice, "freeze_delegate.rs");
      expect(readAsset(first).owner.toString()).to.equal(alice.publicKey.toBase58());
    });

    it("claim_rewards one second before a full day fails", async () => {
      warpTo(stakedAt + DAY - 1);
      expectFail([await claimIx(alice, first)], alice, "NothingToClaim");
    });

    it("claim_rewards at exactly one day pays out and the NFT stays staked and frozen", async () => {
      warpTo(stakedAt + DAY);
      sendOk([await claimIx(alice, first)], alice);

      expect(rewardBalance(alice.publicKey)).to.equal(POINTS_PER_DAY);
      expect(readAsset(first).freezeDelegate.frozen).to.equal(true);
      expect(svm.getAccount(stakePda(first))).to.not.equal(null);
      expect(totalStaked()).to.equal("1");
    });

    it("someone else cant claim your rewards", async () => {
      warpTo(stakedAt + 2 * DAY);
      expectFail([await claimIx(bob, first)], bob, "NotOwner");
    });

    it("unstake one second before the freeze period ends fails", async () => {
      warpTo(stakedAt + FREEZE_PERIOD - 1);
      expectFail([await unstakeIx(alice, first)], alice, "FreezePeriodNotOver");
    });

    it("unstake exactly at the freeze period thaws it, pays the rest and drops total_staked", async () => {
      warpTo(stakedAt + FREEZE_PERIOD);
      sendOk([await unstakeIx(alice, first)], alice);

      // claimed day 1 already, so this pays days 2 through 7
      expect(rewardBalance(alice.publicKey)).to.equal(7 * POINTS_PER_DAY);
      const asset = readAsset(first);
      expect(asset.freezeDelegate).to.equal(undefined);
      expect(asset.burnDelegate).to.equal(undefined);
      expect(svm.getAccount(stakePda(first))).to.equal(null);
      expect(totalStaked()).to.equal("0");
    });

    it("burn_staked_nft burns it through the burn delegate and pays the bonus", async () => {
      const second = await mintAsset(bob, "Staker #2");
      third = await mintAsset(bob, "Staker #3");
      sendOk([await stakeIx(bob, second), await stakeIx(bob, third)], bob);
      expect(totalStaked()).to.equal("2");

      sendOk([await burnIx(bob, second)], bob);

      // burned the same second it was staked, so it's just the bonus, no daily points
      expect(rewardBalance(bob.publicKey)).to.equal(BURN_BONUS);
      // core leaves a 1 byte tombstone behind instead of a real asset
      const leftover = svm.getAccount(second);
      expect(leftover === null || leftover.data.length <= 1).to.equal(true);
      expect(svm.getAccount(stakePda(second))).to.equal(null);
      expect(totalStaked()).to.equal("1");
    });

    it("you cant burn someone else's staked NFT", async () => {
      // bob's #3 is still staked. alice tries to cash it in
      expectFail([await burnIx(alice, third)], alice, "NotOwner");
      expect(readAsset(third).freezeDelegate.frozen).to.equal(true);
      expect(totalStaked()).to.equal("1");
    });
  });

  describe("task 2: oracle that only allows transfers 9 to 5 UTC", () => {
    let nft: PublicKey;
    // a fresh day, well after all the staking stuff
    const D = DAY0 + 20 * DAY;

    before(async () => {
      nft = await mintAsset(alice, "Traveler");
    });

    it("16:59:59 is still open, so the crank changes nothing and earns nothing", async () => {
      expect(await crankAt(D + 17 * HOUR - 1)).to.equal(0);
      expect(oracleTransferState()).to.deep.equal({ pass: {} });
    });

    it("transfer works during market hours", async () => {
      sendOk([await transferIx(alice, bob.publicKey, nft)], alice);
      expect(readAsset(nft).owner.toString()).to.equal(bob.publicKey.toBase58());
    });

    it("17:00:00 exactly is closed. crank flips it to rejected and gets paid", async () => {
      expect(await crankAt(D + 17 * HOUR)).to.equal(CRANK_REWARD);
      expect(oracleTransferState()).to.deep.equal({ rejected: {} });
    });

    it("transfer after hours gets rejected by the oracle", async () => {
      expectFail([await transferIx(bob, alice.publicKey, nft)], bob, CORE_REJECTED);
      expect(readAsset(nft).owner.toString()).to.equal(bob.publicKey.toBase58());
    });

    it("cranking again right after doesnt pay twice", async () => {
      expect(await crankAt(D + 17 * HOUR + 60)).to.equal(0);
    });

    it("08:59:59 next morning is still closed", async () => {
      expect(await crankAt(D + DAY + 9 * HOUR - 1)).to.equal(0);
      expect(oracleTransferState()).to.deep.equal({ rejected: {} });
      expectFail([await transferIx(bob, alice.publicKey, nft)], bob, CORE_REJECTED);
    });

    it("09:00:00 exactly opens it, crank gets paid, transfer goes through", async () => {
      expect(await crankAt(D + DAY + 9 * HOUR)).to.equal(CRANK_REWARD);
      expect(oracleTransferState()).to.deep.equal({ pass: {} });
      sendOk([await transferIx(bob, alice.publicKey, nft)], bob);
      expect(readAsset(nft).owner.toString()).to.equal(alice.publicKey.toBase58());
    });

    it("17:05:00 is the last second of the reward window, still pays", async () => {
      expect(await crankAt(D + DAY + 17 * HOUR + 300)).to.equal(CRANK_REWARD);
      expect(oracleTransferState()).to.deep.equal({ rejected: {} });
    });

    it("09:05:01 is one second too late. state still updates but no reward", async () => {
      expect(await crankAt(D + 2 * DAY + 9 * HOUR + 301)).to.equal(0);
      expect(oracleTransferState()).to.deep.equal({ pass: {} });
    });
  });
});
