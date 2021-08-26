import  {Terra_Merkle_Tree}  from "./terra_merkle_tree.js";
import  {EVM_Merkle_Tree}  from "./evm_merkle_tree.js";
import airdropdataTerra from "./data/userAddressesWithAirdropAmountTerra.json";
import airdropdataEvm from "./data/userAddressesWithAirdropAmountEvm.json";
import utils from 'web3-utils';
import Web3 from 'web3';


const TERRA_MERKLE_ROOTS = 2;
const EVM_MERKLE_ROOTS = 2;

// TERRA ECOSYSTEM AIRDROP :: RETURNS ROOTS OF THE MERKLE TREES FOR TERRA USERS
export async function getMerkleRootsForTerraUsers() { 
    let terra_merkle_roots = [];
    let n = TERRA_MERKLE_ROOTS;
  
    for (let i=0; i<n; i++ ) {
        let terra = prepareDataForMerkleTree(airdropdataTerra.data , i * Math.round(airdropdataTerra.data.length/n) , (i+1) * Math.round(airdropdataTerra.data.length/n)  );
        let airdrop = new Terra_Merkle_Tree(terra);
        let terra_merkle_root = airdrop.getMerkleRoot();
        terra_merkle_roots.push(terra_merkle_root);            
    }
  
    return terra_merkle_roots;
  }
  

// EVM ECOSYSTEM AIRDROP :: RETURNS ROOTS OF THE MERKLE TREES FOR EVM USERS
export async function getMerkleRootsForEVMUsers() { 
    let evm_merkle_roots = [];
    let n = EVM_MERKLE_ROOTS;
  
    for (let i=0; i<n; i++ ) {
        let evm = prepareDataForMerkleTree(airdropdataEvm.data , i * Math.round(airdropdataEvm.data.length/n) , (i+1) * Math.round(airdropdataEvm.data.length/n)  );
        let airdrop = new EVM_Merkle_Tree(evm);
        let evm_merkle_root = airdrop.getMerkleRoot();
        evm_merkle_roots.push(evm_merkle_root);
    }
  
    return evm_merkle_roots;
  }
  

// TERRA ECOSYSTEM AIRDROP :: RETURNS MERKLE PROOF
export function get_Terra_MerkleProof( leaf ) {
    let terra_merkle_trees = [];
    let n = TERRA_MERKLE_ROOTS;
  
    for (let i=0; i<n; i++ ) {
        let terra = prepareDataForMerkleTree(airdropdataTerra.data , i * Math.round(airdropdataTerra.data.length/n) , (i+1) * Math.round(airdropdataTerra.data.length/n)  );
        let terra_merkle_tree = new Terra_Merkle_Tree(terra);
        terra_merkle_trees.push(terra_merkle_tree);            
    }
  
    let proof = [];
    for (let i=0; i<terra_merkle_trees.length; i++ ) {
        proof = terra_merkle_trees[i].getMerkleProof( leaf );
        if (proof.length > 1) {
          return { "proof":proof, "root_index":i }; 
        }
    }
    return { "proof":null, "root_index":-1 }; 
  }  

// EVM ECOSYSTEM AIRDROP :: RETURNS MERKLE PROOF
export function get_EVM_MerkleProof( leaf ) {
    let evm_merkle_trees = [];
    let n = EVM_MERKLE_ROOTS;
  
    for (let i=0; i<n; i++ ) {
      let evm = prepareDataForMerkleTree(airdropdataEvm.data , i * Math.round(airdropdataEvm.data.length/n) , (i+1) * Math.round(airdropdataEvm.data.length/n)  );
      let evm_merkle_tree = new EVM_Merkle_Tree(evm);
      evm_merkle_trees.push(evm_merkle_tree);
    }
  
    let proof = [];
    for (let i=0; i<evm_merkle_trees.length; i++ ) {
        proof = evm_merkle_trees[i].getMerkleProof( leaf );
        if (proof.length > 1) {
          let is_valid = evm_merkle_trees[i].verify(proof,leaf );
          return { "proof":proof, "root_index":i }; 
        }
    }
    return { "proof":null, "root_index":-1 }; 
}

// PREPARE DATA FOR THE MERKLE TREE
export function prepareDataForMerkleTree( data, str, end ) { 
    let dataArray = [];
    for ( let i=str; i < end; i++  ) {  
        let dataObj = JSON.parse( JSON.stringify(data[i]) );
        let ac = { "address":dataObj[0], "amount":dataObj[1].toString() };
        dataArray.push(ac);
    }
    return dataArray;
}

// EVM AIRDROP : SIGN THE MESSAGE
export function get_EVM_Signature(evm_account, msg) {
    var message = utils.isHexStrict(msg) ? utils.hexToUtf8(msg) : msg;
    var ethMessage = "\x19Ethereum Signed Message:\n" + message.length + message;
    let signature =  evm_account.sign(msg);    
    var web3 = new Web3(Web3.givenProvider || 'ws://some.local-or-remote.node:8546');
    let signee = web3.eth.accounts.recover(msg, signature.signature);
    return signature.signature.substr(2,132);
  }