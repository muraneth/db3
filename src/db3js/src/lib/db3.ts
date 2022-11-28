// import * as bip39 from "@scure/bip39";
// import { wordlist } from "@scure/bip39/wordlists/english";
// import { gen_key, sign } from "../pkg/db3_jsdk";
import db3_mutation_pb, { KVPair } from "../pkg/db3_mutation_pb";
import db3_base_pb from "../pkg/db3_base_pb";
import db3_node_pb from "../pkg/db3_node_pb";
import { StorageNodeClient } from "../pkg/Db3_nodeServiceClientPb";

export interface Mutation {
	ns: string;
	gasLimit: number;
	data: Record<string, any>;
}

export interface DB3_Instance {
	submitMutation(mutation: Mutation, signature?: Uint8Array | string): any;
}

export interface DB3_Options {
	mode: "DEV" | "PROD";
}

function encodeUint8Array(text: string) {
	return new TextEncoder().encode(text);
}

export class DB3 {
	private client: StorageNodeClient;
	constructor(node: string, options?: DB3_Options) {
		this.client = new StorageNodeClient(node, null, null);
	}
	submitMutaition(
		mutation: Mutation,
		sign: (target: Uint8Array) => Uint8Array,
	) {
		const kvPairsList: KVPair[] = [];
		Object.keys(mutation.data).forEach((key: string) => {
			const kv_pair = new db3_mutation_pb.KVPair();
			kv_pair.setKey(encodeUint8Array(key));
			kv_pair.setValue(encodeUint8Array(mutation.data[key]));
			kv_pair.setAction(db3_mutation_pb.MutationAction.INSERTKV);
			kvPairsList.push(kv_pair);
		});

		const mutationObj = new db3_mutation_pb.Mutation();
		mutationObj.setNs(encodeUint8Array(mutation.ns));
		mutationObj.setKvPairsList(kvPairsList);
		mutationObj.setNonce(Date.now());
		mutationObj.setChainId(db3_base_pb.ChainId.MAINNET);
		mutationObj.setChainRole(db3_base_pb.ChainRole.STORAGESHARDCHAIN);
		mutationObj.setGasPrice();
		mutationObj.setGas(mutation.gasLimit);

		const signature = sign(mutationObj.serializeBinary());
		const writeRequest = new db3_mutation_pb.WriteRequest();
		writeRequest.setMutation(mutationObj.serializeBinary());
		writeRequest.setSignature(signature);

		const broadcastRequest = new db3_node_pb.BroadcastRequest();
		broadcastRequest.setBody(writeRequest.serializeBinary());

		return this.client
			.broadcast(broadcastRequest, {})
			.then((res) => res.toObject());
	}
}

// export function generateMnemonic() {
// 	return bip39.generateMnemonic(wordlist);
// }

// export function generateKey(mn: string, password: string) {
// 	return bip39.mnemonicToSeed(mn, password).then((seed: Uint8Array) => {
// 		const [pk, sk] = gen_key(seed);
// 		return [pk, sk, seed];
// 	});
// }