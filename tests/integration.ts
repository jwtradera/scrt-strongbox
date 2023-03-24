import axios from "axios";
import { Wallet, SecretNetworkClient } from "secretjs";
import fs from "fs";
import assert from "assert";

let endpoint = "http://localhost:1317";
let chainId = "secretdev-1";

// Generate random string based on length
const randomString = (len: number) => {
  let result = "";
  const characters =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  const charactersLength = characters.length;
  let counter = 0;
  while (counter < len) {
    result += characters.charAt(Math.floor(Math.random() * charactersLength));
    counter += 1;
  }
  return result;
};

// Returns a client with which we can interact with secret network
const initializeClient = (wallet?: Wallet) => {
  if (!wallet) {
    wallet = new Wallet();
  }
  const client = new SecretNetworkClient({
    // Create a client to interact with the network
    url: endpoint,
    chainId: chainId,
    wallet: wallet,
    walletAddress: wallet.address,
  });

  console.log(`Initialized client with wallet address: ${wallet.address}`);
  return client;
};

// Stores and instantiaties a new contract in our network
const initializeContract = async (
  client: SecretNetworkClient,
  contractPath: string
) => {
  const wasmCode = fs.readFileSync(contractPath);
  console.log("Uploading contract");

  const uploadReceipt = await client.tx.compute.storeCode(
    {
      wasm_byte_code: wasmCode,
      sender: client.address,
      source: "",
      builder: "",
    },
    {
      gasLimit: 5000000,
    }
  );

  if (uploadReceipt.code !== 0) {
    console.log(
      `Failed to get code id: ${JSON.stringify(uploadReceipt.rawLog)}`
    );
    throw new Error(`Failed to upload contract`);
  }

  const codeIdKv = uploadReceipt.jsonLog![0].events[0].attributes.find(
    (a: any) => {
      return a.key === "code_id";
    }
  );

  const codeId = Number(codeIdKv!.value);
  console.log("Contract codeId: ", codeId);

  const contractCodeHash = (
    await client.query.compute.codeHashByCodeId({ code_id: String(codeId) })
  ).code_hash;

  if (contractCodeHash === undefined) {
    throw new Error(`Failed to get code hash`);
  }

  console.log(`Contract hash: ${contractCodeHash}`);

  const contract = await client.tx.compute.instantiateContract(
    {
      sender: client.address,
      code_id: codeId,
      init_msg: { serenity_seed: randomString(32) },
      code_hash: contractCodeHash,
      label: "Serenity Strongbox_" + Math.ceil(Math.random() * 10000), // The label should be unique for every contract, add random string in order to maintain uniqueness
    },
    {
      gasLimit: 1000000,
    }
  );

  if (contract.code !== 0) {
    throw new Error(
      `Failed to instantiate the contract with the following error ${contract.rawLog}`
    );
  }

  const contractAddress = contract.arrayLog!.find(
    (log) => log.type === "message" && log.key === "contract_address"
  )!.value;

  console.log(`Contract address: ${contractAddress}`);

  const contractInfo: [string, string] = [contractCodeHash, contractAddress];
  return contractInfo;
};

const getFromFaucet = async (address: string) => {
  await axios.get(`http://localhost:5000/faucet?address=${address}`);
};

async function getScrtBalance(userCli: SecretNetworkClient): Promise<string> {
  let balanceResponse = await userCli.query.bank.balance({
    address: userCli.address,
    denom: "uscrt",
  });

  if (balanceResponse?.balance?.amount === undefined) {
    throw new Error(`Failed to get balance for address: ${userCli.address}`);
  }

  return balanceResponse.balance.amount;
}

async function fillUpFromFaucet(
  client: SecretNetworkClient,
  targetBalance: Number
) {
  let balance = await getScrtBalance(client);
  while (Number(balance) < targetBalance) {
    try {
      await getFromFaucet(client.address);
    } catch (e) {
      console.error(`failed to get tokens from faucet: ${e}`);
    }
    balance = await getScrtBalance(client);
  }
  console.error(`got tokens from faucet: ${balance}`);
}

// Initialization procedure
async function initializeAndUploadContract() {
  const client = await initializeClient();

  await fillUpFromFaucet(client, 100_000_000);

  const [contractHash, contractAddress] = await initializeContract(
    client,
    "../contract.wasm"
  );

  var clientInfo: [SecretNetworkClient, string, string] = [
    client,
    contractHash,
    contractAddress,
  ];
  return clientInfo;
}

async function queryStrongbox(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddress: string,
  behalf: string,
  key: string
): Promise<string> {
  type StrongboxResponse = { strongbox: string };

  const response = (await client.query.compute.queryContract({
    contract_address: contractAddress,
    code_hash: contractHash,
    query: {
      get_strongbox: {
        behalf,
        key,
      },
    },
  })) as StrongboxResponse;

  if (response.strongbox) {
    return response.strongbox;
  } else {
    throw new Error(JSON.stringify(response));
  }
}

async function updateStrongboxTx(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddess: string,
  strongbox: string
) {
  const tx = await client.tx.compute.executeContract(
    {
      sender: client.address,
      contract_address: contractAddess,
      code_hash: contractHash,
      msg: {
        update_strongbox: {
          strongbox,
        },
      },
      sent_funds: [],
    },
    {
      gasLimit: 200000,
    }
  );

  console.log(`Update Strongbox TX used ${tx.gasUsed} gas`);
}

async function createViewingKeyTx(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddess: string,
  viewer: string
): Promise<string> {
  const tx = await client.tx.compute.executeContract(
    {
      sender: client.address,
      contract_address: contractAddess,
      code_hash: contractHash,
      msg: {
        create_viewing_key: {
          viewer,
          entropy: randomString(20),
          padding: "",
        },
      },
      sent_funds: [],
    },
    {
      gasLimit: 200000,
    }
  );

  console.log(`Create ViewingKey TX used ${tx.gasUsed} gas`);

  // Trim viewing key
  const key_str = tx.data.toString().trim();
  const viewing_key = key_str.substring(2, key_str.length - 1);
  return viewing_key;
}

async function transferOwnershipTx(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddess: string,
  owner: string
) {
  const tx = await client.tx.compute.executeContract(
    {
      sender: client.address,
      contract_address: contractAddess,
      code_hash: contractHash,
      msg: {
        transfer_ownership: {
          new_owner: owner,
        },
      },
      sent_funds: [],
    },
    {
      gasLimit: 200000,
    }
  );

  console.log(`Transfer Ownership TX used ${tx.gasUsed} gas`);
}

async function revokeViewingKeyTx(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddess: string,
  viewer: string
) {
  const tx = await client.tx.compute.executeContract(
    {
      sender: client.address,
      contract_address: contractAddess,
      code_hash: contractHash,
      msg: {
        revoke_viewing_key: {
          viewer,
        },
      },
      sent_funds: [],
    },
    {
      gasLimit: 200000,
    }
  );

  console.log(`Revoke Viewing Key TX used ${tx.gasUsed} gas`);
}

async function test_update_strongbox(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddress: string
) {
  // Update strongbox
  const strongbox = "test strongbox #1";
  await updateStrongboxTx(client, contractHash, contractAddress, strongbox);

  // Prepare viewer and create viewing key
  const viewer1 = new Wallet();
  const viewer1_key = await createViewingKeyTx(
    client,
    contractHash,
    contractAddress,
    viewer1.address
  );

  // Query strongbox with key
  const strongbox_result: string = await queryStrongbox(
    client,
    contractHash,
    contractAddress,
    viewer1.address,
    viewer1_key
  );

  assert.equal(strongbox, strongbox_result, "Strongbox not matched");
}

async function test_transfer_ownership(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddress: string
) {
  // Update strongbox
  let strongbox = "test strongbox #1";
  await updateStrongboxTx(client, contractHash, contractAddress, strongbox);

  // Prepare new admin and transfer ownership
  const new_owner = new Wallet();
  await transferOwnershipTx(
    client,
    contractHash,
    contractAddress,
    new_owner.address
  );

  // Update client
  const new_client = initializeClient(new_owner);
  await fillUpFromFaucet(new_client, 100_000_000);

  // Try to update strongbox with new owner
  strongbox = "test strongbox #2";
  await updateStrongboxTx(new_client, contractHash, contractAddress, strongbox);

  // Prepare viewer and create viewing key
  const viewer1 = new Wallet();
  const viewer1_key = await createViewingKeyTx(
    new_client,
    contractHash,
    contractAddress,
    viewer1.address
  );

  // Query strongbox with key
  const strongbox_result: string = await queryStrongbox(
    new_client,
    contractHash,
    contractAddress,
    viewer1.address,
    viewer1_key
  );

  assert.equal(strongbox, strongbox_result, "Strongbox not matched");
}

async function test_revoke_viewing_key(
  client: SecretNetworkClient,
  contractHash: string,
  contractAddress: string
) {
  // Update strongbox
  const strongbox = "test strongbox #1";
  await updateStrongboxTx(client, contractHash, contractAddress, strongbox);

  // Prepare viewer and create viewing key
  const viewer1 = new Wallet();
  const viewer1_key = await createViewingKeyTx(
    client,
    contractHash,
    contractAddress,
    viewer1.address
  );

  // Revoke viewing key
  await revokeViewingKeyTx(
    client,
    contractHash,
    contractAddress,
    viewer1.address
  );

  // Try to query strongbox with key
  try {
    await queryStrongbox(
      client,
      contractHash,
      contractAddress,
      viewer1.address,
      viewer1_key
    );
    throw Error("Query strongbox with revoked key should be failed");
  } catch (ex) {
    console.log(ex);
    assert.ok(
      (ex as Error).message.includes("Your viewing key does not matched"),
      "Error message is not valid"
    );
  }
}

async function runTestFunction(
  tester: (
    client: SecretNetworkClient,
    contractHash: string,
    contractAddress: string
  ) => void,
  client: SecretNetworkClient,
  contractHash: string,
  contractAddress: string
) {
  console.log(`Testing ${tester.name}`);
  await tester(client, contractHash, contractAddress);
  console.log(`[SUCCESS] ${tester.name}`);
}

(async () => {
  const [client, contractHash, contractAddress] =
    await initializeAndUploadContract();

  await runTestFunction(
    test_update_strongbox,
    client,
    contractHash,
    contractAddress
  );
  await runTestFunction(
    test_revoke_viewing_key,
    client,
    contractHash,
    contractAddress
  );
  await runTestFunction(
    test_transfer_ownership,
    client,
    contractHash,
    contractAddress
  );
})();
