import {
  Contract,
  Networks,
  TransactionBuilder,
  BASE_FEE,
  rpc,
  xdr,
  scValToNative,
  nativeToScVal,
  Address,
} from "@stellar/stellar-sdk";

export const CONTRACT_ID = process.env.NEXT_PUBLIC_CONTRACT_ID ?? "";
export const RPC_URL =
  process.env.NEXT_PUBLIC_RPC_URL ?? "https://soroban-testnet.stellar.org";
export const NETWORK_PASSPHRASE =
  process.env.NEXT_PUBLIC_NETWORK_PASSPHRASE ?? Networks.TESTNET;

export type TaskStatus = "Open" | "Claimed" | "Refunded";

export interface EscrowTask {
  taskId: bigint;
  employer: string;
  worker: string;
  token: string;
  amount: bigint;
  vkHash: string;   // hex
  deadline: bigint; // unix seconds
  status: TaskStatus;
}

// ── helpers ──────────────────────────────────────────────────────────────────

function server() {
  return new rpc.Server(RPC_URL, { allowHttp: false });
}

async function buildTx(
  sourcePublicKey: string,
  operation: xdr.Operation
): Promise<string> {
  const s = server();
  const account = await s.getAccount(sourcePublicKey);
  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(operation)
    .setTimeout(30)
    .build();

  const prepared = await s.prepareTransaction(tx);
  return prepared.toXDR();
}

// ── contract calls ────────────────────────────────────────────────────────────

export async function getTask(taskId: bigint): Promise<EscrowTask | null> {
  const s = server();
  const contract = new Contract(CONTRACT_ID);

  const result = await s.simulateTransaction(
    new TransactionBuilder(
      await s.getAccount(
        "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN" // read-only dummy
      ),
      { fee: BASE_FEE, networkPassphrase: NETWORK_PASSPHRASE }
    )
      .addOperation(
        contract.call("get_task", nativeToScVal(taskId, { type: "u64" }))
      )
      .setTimeout(30)
      .build()
  );

  if (rpc.Api.isSimulationError(result)) return null;

  const val = (result as rpc.Api.SimulateTransactionSuccessResponse)
    .result?.retval;
  if (!val || val.switch().name === "scvVoid") return null;

  return parseTask(taskId, scValToNative(val));
}

export async function buildCreateTask(
  employer: string,
  worker: string,
  token: string,
  amount: bigint,
  vkHash: string, // 32-byte hex
  deadlineUnix: bigint
): Promise<string> {
  const contract = new Contract(CONTRACT_ID);
  const vkHashBytes = Buffer.from(vkHash, "hex");

  return buildTx(
    employer,
    contract.call(
      "create_task",
      new Address(employer).toScVal(),
      new Address(worker).toScVal(),
      new Address(token).toScVal(),
      nativeToScVal(amount, { type: "i128" }),
      xdr.ScVal.scvBytes(vkHashBytes),
      nativeToScVal(deadlineUnix, { type: "u64" })
    )
  );
}

export async function buildClaimReward(
  worker: string,
  taskId: bigint,
  vkBytes: string,   // hex
  vkScVal: xdr.ScVal,
  proofScVal: xdr.ScVal,
  publicInputsScVal: xdr.ScVal
): Promise<string> {
  const contract = new Contract(CONTRACT_ID);
  return buildTx(
    worker,
    contract.call(
      "claim_reward",
      new Address(worker).toScVal(),
      nativeToScVal(taskId, { type: "u64" }),
      xdr.ScVal.scvBytes(Buffer.from(vkBytes, "hex")),
      vkScVal,
      proofScVal,
      publicInputsScVal
    )
  );
}

export async function buildRefund(
  employer: string,
  taskId: bigint
): Promise<string> {
  const contract = new Contract(CONTRACT_ID);
  return buildTx(
    employer,
    contract.call(
      "refund",
      new Address(employer).toScVal(),
      nativeToScVal(taskId, { type: "u64" })
    )
  );
}

export async function submitSignedTx(signedXdr: string): Promise<string> {
  const s = server();
  const tx = TransactionBuilder.fromXDR(signedXdr, NETWORK_PASSPHRASE);
  const response = await s.sendTransaction(tx);
  if (response.status === "ERROR") throw new Error(response.errorResult?.toString());
  return response.hash;
}

// ── parsers ───────────────────────────────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function parseTask(taskId: bigint, raw: any): EscrowTask {
  return {
    taskId,
    employer: raw.employer,
    worker: raw.worker,
    token: raw.token,
    amount: BigInt(raw.amount),
    vkHash: Buffer.from(raw.vk_hash).toString("hex"),
    deadline: BigInt(raw.deadline),
    status: raw.status as TaskStatus,
  };
}
