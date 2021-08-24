/*
Script to deploy a cw20 token from a multisig account using the mars-minter contract as the token minter.

This script is designed to work with Terra Columbus-4.

Dependencies:
  - rust
  - terracli 58602320d2907814cfccdf43e9679468bb4bd8d3
  - cosmwasm-plus v0.2.0
  - mars-minter
  - Add accounts and multisig to terracli
  - Set environment variables in a .env file (see below for details of the required variables)

Dependencies to run on LocalTerra:
  - docker
  - LocalTerra 1c3f42a60116b4c17cb5d002aa194eae9b8811b5
*/

import {
  isTxError,
  LCDClient,
  LocalTerra,
  MsgExecuteContract,
  MsgSend,
  MsgUpdateContractOwner,
  StdTx,
  Wallet
} from "@terra-money/terra.js"
import { CLIKey } from "@terra-money/terra.js/dist/key/CLIKey.js"
import { strictEqual } from "assert"
import { execSync } from "child_process"
import { unlinkSync, writeFileSync } from "fs"
import 'dotenv/config.js'
import {
  createTransaction,
  executeContract,
  instantiateContract,
  performTransaction,
  queryContract,
  recover,
  setTimeoutDuration,
  uploadContract
} from "./helpers.js"

// Required environment variables:

// All:
const MULTISIG_ADDRESS = process.env.MULTISIG_ADDRESS!
// Name of the multisig in terracli
const MULTISIG_NAME = process.env.MULTISIG_NAME!
// Names of the multisig keys in terracli
const MULTISIG_KEYS = process.env.MULTISIG_KEYS!.split(",")
const MULTISIG_THRESHOLD = parseInt(process.env.MULTISIG_THRESHOLD!)

// Testnet:
const CHAIN_ID = process.env.CHAIN_ID
const LCD_CLIENT_URL = process.env.LCD_CLIENT_URL
const CW20_CODE_ID = process.env.CW20_CODE_ID
const MARS_MINTER_CODE_ID = process.env.MARS_MINTER_CODE_ID

// LocalTerra:
const CW20_BINARY_PATH = process.env.CW20_BINARY_PATH
const MARS_MINTER_BINARY_PATH = process.env.MARS_MINTER_BINARY_PATH!

// Main

async function main() {
  const isTestnet = CHAIN_ID !== undefined

  let terra: LCDClient | LocalTerra
  let wallet: Wallet
  let cw20CodeID: number
  let marsMinterCodeID: number

  if (isTestnet) {
    terra = new LCDClient({
      URL: LCD_CLIENT_URL!,
      chainID: CHAIN_ID!
    })

    wallet = recover(terra, process.env.WALLET!)

    cw20CodeID = parseInt(CW20_CODE_ID!)
    marsMinterCodeID = parseInt(MARS_MINTER_CODE_ID!)

  } else {
    setTimeoutDuration(0)

    terra = new LocalTerra()

    wallet = (terra as LocalTerra).wallets.test1

    // Upload contract code
    cw20CodeID = await uploadContract(terra, wallet, CW20_BINARY_PATH!)
    console.log(cw20CodeID)
    marsMinterCodeID = await uploadContract(terra, wallet, MARS_MINTER_BINARY_PATH!)
    console.log(marsMinterCodeID)
  }

  const multisig = new Wallet(terra, new CLIKey({ keyName: MULTISIG_NAME }))

  // Instantiate mars-minter
  const minterAddress = await instantiateContract(terra, wallet, marsMinterCodeID, { admins: [wallet.key.accAddress, MULTISIG_ADDRESS] })
  console.log("minter:", minterAddress)

  // Token info
  const TOKEN_NAME = "Mars"
  const TOKEN_SYMBOL = "MARS"
  const TOKEN_DECIMALS = 6
  // The minter address cannot be changed after the contract is instantiated
  const TOKEN_MINTER = minterAddress
  // The cap cannot be changed after the contract is instantiated
  const TOKEN_CAP = 1_000_000_000_000000
  // TODO check if we want initial balances in prod
  const TOKEN_INITIAL_AMOUNT = 1_000_000_000000
  const TOKEN_INITIAL_AMOUNT_ADDRESS = TOKEN_MINTER

  const TOKEN_INFO = {
    name: TOKEN_NAME,
    symbol: TOKEN_SYMBOL,
    decimals: TOKEN_DECIMALS,
    initial_balances: [
      {
        address: TOKEN_INITIAL_AMOUNT_ADDRESS,
        amount: String(TOKEN_INITIAL_AMOUNT)
      }
    ],
    mint: {
      minter: TOKEN_MINTER,
      cap: String(TOKEN_CAP)
    }
  }

  // Instantiate Mars token contract
  const marsAddress = await instantiateContract(terra, wallet, cw20CodeID, TOKEN_INFO)
  console.log("mars:", marsAddress)
  console.log(await queryContract(terra, marsAddress, { token_info: {} }))
  console.log(await queryContract(terra, marsAddress, { minter: {} }))

  let balance = await queryContract(terra, marsAddress, { balance: { address: TOKEN_INFO.initial_balances[0].address } })
  strictEqual(balance.balance, TOKEN_INFO.initial_balances[0].amount)

  // Add Mars token address to mars-minter config
  await executeContract(terra, wallet, minterAddress, { update_config: { config: { mars_token_address: marsAddress } } })

  const newConfig = await queryContract(terra, minterAddress, { config: {} })
  strictEqual(newConfig.mars_token_address, marsAddress)

  // Remove wallet from mars-minter admins
  await executeContract(terra, wallet, minterAddress, { update_admins: { admins: [MULTISIG_ADDRESS] } })

  const walletCanMint = await queryContract(terra, minterAddress, { can_mint: { sender: wallet.key.accAddress } })
  strictEqual(walletCanMint.can_mint, false)

  const multisigCanMint = await queryContract(terra, minterAddress, { can_mint: { sender: MULTISIG_ADDRESS } })
  strictEqual(multisigCanMint.can_mint, true)

  // Update mars-minter owner
  const newOwner = MULTISIG_ADDRESS

  await performTransaction(terra, wallet, new MsgUpdateContractOwner(wallet.key.accAddress, newOwner, minterAddress))

  const minterContractInfo = await terra.wasm.contractInfo(minterAddress)
  strictEqual(minterContractInfo.owner, newOwner)

  // Update Mars token owner
  await performTransaction(terra, wallet, new MsgUpdateContractOwner(wallet.key.accAddress, newOwner, marsAddress))

  const marsContractInfo = await terra.wasm.contractInfo(marsAddress)
  strictEqual(marsContractInfo.owner, newOwner)

  // Mint tokens
  // NOTE this is for testnet use only -- do not mint tokens like this on mainnet
  const mintAmount = 1_000_000000
  const recipient = wallet.key.accAddress

  // Send coins to the multisig address. On testnet, use the faucet to initialise the multisig balance.
  if (!isTestnet) {
    await performTransaction(terra, wallet, new MsgSend(
      wallet.key.accAddress,
      MULTISIG_ADDRESS,
      { uluna: 1_000_000000, uusd: 1_000_000000 }
    ))
  }

  // Create an unsigned tx
  const mintMsg = { mint: { recipient: recipient, amount: String(mintAmount) } }
  const tx = await createTransaction(terra, multisig, new MsgExecuteContract(MULTISIG_ADDRESS, minterAddress, mintMsg))
  writeFileSync('unsigned_tx.json', tx.toStdTx().toJSON())

  // Create K of N signatures for the tx
  let fns: Array<string> = []
  for (const key of MULTISIG_KEYS.slice(0, MULTISIG_THRESHOLD)) {
    const cli = new CLIKey({ keyName: key, multisig: MULTISIG_ADDRESS })
    const sig = await cli.createSignature(tx)

    const fn = `${key}_sig.json`
    writeFileSync(fn, sig.toJSON())
    fns.push(fn)
  }

  // Create a signed tx by aggregating the K signatures
  const signedTxData = execSync(
    `terracli tx multisign unsigned_tx.json ${MULTISIG_NAME} ${fns.join(" ")} ` +
    `--offline ` +
    `--chain-id ${tx.chain_id} --account-number ${tx.account_number} --sequence ${tx.sequence} `,
    { encoding: 'utf-8' }
  )

  // Broadcast the tx
  const signedTx = StdTx.fromData(JSON.parse(signedTxData.toString()))
  const result = await terra.tx.broadcast(signedTx);
  if (isTxError(result)) {
    throw new Error(
      `transaction failed. code: ${result.code}, codespace: ${result.codespace}, raw_log: ${result.raw_log}`
    );
  }

  const tokenInfo = await queryContract(terra, marsAddress, { token_info: {} })
  console.log(tokenInfo)
  strictEqual(tokenInfo.total_supply, String(TOKEN_INITIAL_AMOUNT + mintAmount))

  balance = await queryContract(terra, marsAddress, { balance: { address: recipient } })
  console.log(balance)
  strictEqual(balance.balance, String(mintAmount))

  // Remove tmp files
  for (const fn of [...fns, "unsigned_tx.json"]) {
    unlinkSync(fn)
  }

  console.log("OK")
}

main().catch(err => console.log(err))
