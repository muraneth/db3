//
// bill_sdk.rs
// Copyright (C) 2022 db3.network Author imotai <codego.me@gmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use bytes::BytesMut;
use db3_crypto::signer::Db3Signer;
use db3_proto::db3_account_proto::Account;
use db3_proto::db3_bill_proto::Bill;
use db3_proto::db3_node_proto::{
    storage_node_client::StorageNodeClient, BatchGetKey, BatchGetValue, GetAccountRequest,
    GetKeyRequest, GetSessionInfoRequest, QueryBillRequest, QuerySessionInfo,
    RestartSessionRequest,
};
use db3_session::session_manager::SessionManager;
use ethereum_types::Address as AccountAddress;
use prost::Message;
use std::sync::Arc;
use tonic::Status;

pub struct StoreSDK {
    client: Arc<StorageNodeClient<tonic::transport::Channel>>,
    signer: Db3Signer,
    session: SessionManager,
}

impl StoreSDK {
    pub fn new(
        client: Arc<StorageNodeClient<tonic::transport::Channel>>,
        signer: Db3Signer,
    ) -> Self {
        Self {
            client,
            signer,
            session: SessionManager::new(),
        }
    }

    pub async fn restart_session(&mut self) -> std::result::Result<(String, i32), Status> {
        match self.validate_query_session() {
            Ok(_) => {
                let query_session_info = self.session.get_session_info();
                let mut buf = BytesMut::with_capacity(1024 * 8);
                query_session_info
                    .encode(&mut buf)
                    .map_err(|e| Status::internal(format!("{}", e)))?;
                let buf = buf.freeze();
                let signature = self
                    .signer
                    .sign(buf.as_ref())
                    .map_err(|e| Status::internal(format!("{:?}", e)))?;
                let r = RestartSessionRequest {
                    query_session_info: buf.as_ref().to_vec(),
                    signature,
                };
                let request = tonic::Request::new(r);

                let mut client = self.client.as_ref().clone();
                let response = client.restart_query_session(request).await?.into_inner();
                let old_session_info = format!("{:?}", self.session);
                self.session = SessionManager::create_session(response.session);
                Ok((old_session_info, response.session))
            }
            Err(e) => Err(Status::internal(format!("{}", e))),
        }
    }

    fn validate_query_session(&self) -> std::result::Result<(), Status> {
        Ok(())
    }

    pub async fn get_bills_by_block(
        &mut self,
        height: u64,
        start: u64,
        end: u64,
    ) -> std::result::Result<Vec<Bill>, Status> {
        if self.session.check_session_running() {
            let mut client = self.client.as_ref().clone();
            let q_req = QueryBillRequest {
                height,
                start_id: start,
                end_id: end,
            };
            let request = tonic::Request::new(q_req);
            let response = client.query_bill(request).await?.into_inner();
            self.session.increase_query(1);
            Ok(response.bills)
        } else {
            return Err(Status::permission_denied(
                "Fail to query bill in this session. Please restart query session",
            ));
        }
    }

    pub async fn get_account(&self, addr: &AccountAddress) -> std::result::Result<Account, Status> {
        let r = GetAccountRequest {
            addr: format!("{:?}", addr),
        };
        let request = tonic::Request::new(r);
        let mut client = self.client.as_ref().clone();
        let account = client.get_account(request).await?.into_inner();
        Ok(account)
    }

    pub async fn get_session_info(
        &self,
        addr: &AccountAddress,
    ) -> std::result::Result<QuerySessionInfo, Status> {
        let r = GetSessionInfoRequest {
            addr: format!("{:?}", addr),
        };
        let request = tonic::Request::new(r);
        let mut client = self.client.as_ref().clone();

        let response = client.get_session_info(request).await?.into_inner();
        Ok(response.session_info.unwrap())
    }

    pub async fn batch_get(
        &mut self,
        ns: &[u8],
        keys: Vec<Vec<u8>>,
    ) -> std::result::Result<Option<BatchGetValue>, Status> {
        if self.session.check_session_running() {
            let batch_keys = BatchGetKey {
                ns: ns.to_vec(),
                keys,
                session: 1,
            };
            let mut buf = BytesMut::with_capacity(1024 * 8);
            batch_keys
                .encode(&mut buf)
                .map_err(|e| Status::internal(format!("{}", e)))?;
            let buf = buf.freeze();
            let signature = self
                .signer
                .sign(buf.as_ref())
                .map_err(|e| Status::internal(format!("{:?}", e)))?;
            let r = GetKeyRequest {
                batch_get: buf.as_ref().to_vec(),
                signature,
            };
            let request = tonic::Request::new(r);

            let mut client = self.client.as_ref().clone();
            let response = client.get_key(request).await?.into_inner();
            // TODO(cj): batch keys query should be count as a query or multi queries?
            self.session.increase_query(1);
            Ok(response.batch_get_values)
        } else {
            return Err(Status::permission_denied(
                "Fail to query in this session. Please restart query session",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Db3Signer;
    use super::StoreSDK;
    use crate::mutation_sdk::MutationSDK;
    use db3_proto::db3_base_proto::{ChainId, ChainRole};
    use db3_proto::db3_mutation_proto::KvPair;
    use db3_proto::db3_mutation_proto::{Mutation, MutationAction};
    use db3_proto::db3_node_proto::storage_node_client::StorageNodeClient;
    use fastcrypto::secp256k1::Secp256k1KeyPair;
    use fastcrypto::traits::KeyPair;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::sync::Arc;
    use std::time;
    use tendermint_rpc::HttpClient;
    use tonic::transport::Endpoint;
    #[tokio::test]
    async fn it_get_bills() {
        {
            let client = HttpClient::new("http://127.0.0.1:26657").unwrap();
            let mut rng = StdRng::from_seed([0; 32]);
            let kp = Secp256k1KeyPair::generate(&mut rng);
            let signer = Db3Signer::new(kp);
            let msdk = MutationSDK::new(client, signer);
            let kv = KvPair {
                key: format!("kkkkk_tt{}", 1).as_bytes().to_vec(),
                value: format!("vkalue_tt{}", 1).as_bytes().to_vec(),
                action: MutationAction::InsertKv.into(),
            };
            let mutation = Mutation {
                ns: "my_twitter".as_bytes().to_vec(),
                kv_pairs: vec![kv],
                nonce: 11000,
                chain_id: ChainId::MainNet.into(),
                chain_role: ChainRole::StorageShardChain.into(),
                gas_price: None,
                gas: 10,
            };
            let result = msdk.submit_mutation(&mutation).await;
            assert!(result.is_ok());
            let ten_millis = time::Duration::from_millis(1000);
            std::thread::sleep(ten_millis);
        }

        let mut rng = StdRng::from_seed([0; 32]);
        let kp = Secp256k1KeyPair::generate(&mut rng);
        let signer = Db3Signer::new(kp);
        let ep = "http://127.0.0.1:26659";
        let rpc_endpoint = Endpoint::new(ep.to_string()).unwrap();
        let channel = rpc_endpoint.connect_lazy();
        let client = Arc::new(StorageNodeClient::new(channel));
        let mut sdk = StoreSDK::new(client, signer);
        let result = sdk.get_bills_by_block(1, 0, 10).await;
        if let Err(ref e) = result {
            println!("{}", e);
            assert!(false);
        }
        assert!(result.is_ok());
    }
}
