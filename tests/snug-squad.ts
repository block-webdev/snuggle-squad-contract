import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { SnugSquad } from "../target/types/snug_squad";

describe("snug-squad", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.SnugSquad as Program<SnugSquad>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
