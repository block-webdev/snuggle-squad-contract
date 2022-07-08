import { publicKey } from "@project-serum/anchor/dist/cjs/utils";
import { PublicKey } from "@solana/web3.js";

export const RS_PREFIX = "puffu-nft-staking";
export const RS_STAKEINFO_SEED = "puffu-stake-info";
export const RS_STAKE_SEED = "puffu-nft-staking";
export const RS_VAULT_SEED = "puffu-vault";

export const CLASS_TYPES = [10, 15, 25];
export const LOCK_DAY = [0, 14, 30];
export const REWARDS_BY_RARITY = [0, 3, 6, 10, 15];

export const NETWORK = "devnet";
// devnet
export const SWRD_TOKEN_MINT = new PublicKey(
    "5HkxgJ2JPtTTGJZ4r2HAETpNtkotWirte7CXQ32qyELS"
)

export const NFT_CREATOR = new PublicKey(
    "7etbqNa25YWWQztHrwuyXtG39WnAqPszrGRZmEBPvFup"
);

export const PROGRAM_ID = new PublicKey(
    "B4YcRtUPTGkGctJmdVvXiWgMhPW8iaKHjYcesvH2LzaM"
)

// mainnet
// export const SWRD_TOKEN_MINT = new PublicKey(
//     "ExLjCck16LmtH87hhCAmTk4RWv7getYQeGhLvoEfDLrH"
// )

// export const NFT_CREATOR = new PublicKey(
//     "6rQse6Jq81nBork8x9UwccJJh4qokVVSYujhQRuQgnna"
// );

// export const PROGRAM_ID = new PublicKey(
//     "6RhXNaW1oQYQmjTc1ypb4bEFe1QasPAgEfFNhQ3HnSqo"
// )