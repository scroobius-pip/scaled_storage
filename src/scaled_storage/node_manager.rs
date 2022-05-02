use crate::node::Node;
use ic_cdk::export::{
    candid::{CandidType, Deserialize},
    Principal,
};

use ic_kit::candid::{Decode, Encode};
use ic_kit::ic;
use ic_kit::interfaces::management::{self, CanisterSettings};
use ic_kit::interfaces::Method;
use serde::de::DeserializeOwned;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum CanisterManagerEvent {
    NodeCreated(Principal),
    NodeDeleted(Principal),
    Migrate(MigrateArgs),
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct InstallArgs {
    pub all_nodes: Vec<Principal>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct WasmInitArgs {
    pub position: usize, // 0 for start chunk, 1 for intermediate chunk, 2 for end chunks
    pub wasm_chunk: Vec<u8>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct InitCanisterManagerParam {
    pub args: Option<InstallArgs>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum NodeStatus {
    Initialized,
    Ready,
    Error(NodeError),
    ShutDown,
    Migrating,
    ScaleUp,
    ScaleDown,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum NodeError {
    Migration(String),
    ScaleUp(String),
    Initialize(String),
    Broadcast(String),
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct NodeInfo {
    pub all_nodes: Vec<String>,
    pub current_memory_usage: u64,
    pub status: NodeStatus,
}

#[derive(CandidType, Deserialize)]
struct DataChunk<Data>
where
    Data: CandidType,
{
    data: Vec<(String, Data)>,
}

impl<Data> DataChunk<Data>
where
    Data: CandidType + DeserializeOwned,
{
    fn new(data: Vec<(String, Data)>) -> Self {
        Self { data }
    }

    fn encode(self) -> Result<Vec<u8>, String> {
        // encode_args((self,)).map_err(|_| ())
        Encode!(&self).map_err(|e| e.to_string())
    }

    fn decode(data: &Vec<u8>) -> Result<Self, String> {
        Decode!(data, DataChunk<Data>).map_err(|e| e.to_string())
    }
}

impl<Data> From<&Vec<u8>> for DataChunk<Data>
where
    Data: CandidType + DeserializeOwned,
{
    fn from(data: &Vec<u8>) -> Self {
        Decode!(data, DataChunk<Data>).unwrap()
    }
}

#[derive(CandidType, Deserialize, Debug, Clone)]
pub struct MigrateArgs {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

type Canister<Data> = Node<Principal, Data>;

pub struct CanisterManager<Data: Default + Clone> {
    status: NodeStatus,
    pub canister: Canister<Data>,
    wasm_binary: Option<Vec<u8>>,
}

impl<Data: Default + Clone + CandidType + DeserializeOwned> CanisterManager<Data> {
    pub fn new(node_id: Principal) -> Self {
        let mut new_canister: Node<Principal, Data> =
            Node::new(node_id.clone(), Default::default());

        new_canister.add_node(node_id);

        Self {
            status: NodeStatus::Initialized,
            canister: new_canister,
            wasm_binary: None, // reserve_memory: 0,
        }
    }

    fn get_status(&self) -> &NodeStatus {
        &self.status
    }

    fn should_scale_up(&self) -> bool {
        // let current_memory_usage: u64 = 0;
        // current_memory_usage >= self.max_memory
        //     && self.canister.next_node_id.is_none()
        //     && matches!(self.status, NodeStatus::Ready)
        // let size = self.canister.with_data_mut()
        self.canister.size() > 2 && self.canister.next_node_id.is_none() && matches!(self.status, NodeStatus::Ready)
    }

    fn should_scale_down(&self) -> bool {
        false
        // if self.canister.prev_node_id.is_some() {
        //     let current_memory_usage: u64 = 0;
        //     current_memory_usage <= self.min_memory
        //         && self.canister.prev_node_id.is_some()
        //         && matches!(self.status, NodeStatus::Ready)
        // } else {
        //     false
        // }
    }

    pub fn lifecycle_init_wasm(&mut self, args: WasmInitArgs) -> bool {
        match args.position {
            0 => {
                self.wasm_binary = Some(args.wasm_chunk);
                true
            }
            1 | 2 => match self.wasm_binary.as_mut() {
                Some(wasm_binary) => {
                    wasm_binary.extend_from_slice(&args.wasm_chunk);
                    if args.position == 2 {
                        self.status = NodeStatus::Ready;
                    }
                    true
                }

                None => false,
            },
            _ => false,
        }
    }

    pub async fn lifecyle_init_node(&mut self, all_nodes: Option<Vec<Principal>>) -> () {
        let node_id = self.canister.id;
        let mut new_canister: Node<Principal, Data> = Node::new(node_id, Default::default());

        if let Some(mut all_nodes) = all_nodes {
            let prev_node_id = all_nodes[all_nodes.len() - 1].clone();
            new_canister.prev_node_id = Some(prev_node_id);
            all_nodes.push(node_id);
            for principal_id in all_nodes {
                new_canister.add_node(principal_id);
            }
        }

        self.canister = new_canister;

        self.broadcast_event(CanisterManagerEvent::NodeCreated(self.canister.id))
            .await;
    }

    pub async fn lifecyle_heartbeat_node(&mut self) -> () {
        if self.should_scale_up() {
            self.status = NodeStatus::ScaleUp;
            let create_node_result = self.create_node().await;

            match create_node_result {
                Some(new_node_id) => {
                    self.canister.add_node(new_node_id.clone());
                    let result = self.initialize_node(new_node_id.clone()).await;
                    if !result {
                        self.canister.remove_node(&new_node_id);
                        self.status = NodeStatus::Error(NodeError::Initialize(format!(
                            "Failed to initialize node {}",
                            new_node_id
                        )));

                        return;
                    }
                    self.status = NodeStatus::Migrating;
                    let result = self.migrate_data(new_node_id).await;

                    if !result {
                        self.canister.remove_node(&new_node_id);
                        self.status = NodeStatus::Error(NodeError::Migration(format!(
                            "Failed to migrate data to node {}",
                            new_node_id
                        )));
                        return 
                    }

                    self.status = NodeStatus::Ready;
                    self.canister.next_node_id = Some(new_node_id);
                    self.broadcast_event(CanisterManagerEvent::NodeCreated(new_node_id))
                        .await;
                }
                None => {
                    self.status =
                        NodeStatus::Error(NodeError::ScaleUp("Failed to create node".to_string()));
                }
            }
        } else if self.should_scale_down() {
            // self.status = NodeStatus::ScaleDown;
            // self.broadcast_event(CanisterManagerEvent::NodeDeleted(self.canister.id));
        }
    }

    async fn create_node(&mut self) -> Option<Principal> {
        let arg = management::CreateCanisterArgument {
            settings: Some(CanisterSettings {
                compute_allocation: None,
                controllers: Some(vec![self.canister.id]),
                freezing_threshold: None,
                memory_allocation: None, // reserve_memory: self.reserve_memory,
            }),
        };

        let result = management::CreateCanister::perform_with_payment(
            Principal::management_canister(),
            (arg,),
            10_000_000,
        )
        .await;

        match result {
            Ok((result,)) => Some(result.canister_id),
            Err(_) => None,
        }
    }

    async fn initialize_node(&mut self, canister_id: Principal) -> bool {
        //vector of &Principal to Principal

        let wasm_code = self.wasm_binary.clone().unwrap();

        let install_args = management::InstallCodeArgument {
            canister_id,
            mode: management::InstallMode::Install,
            wasm_module: wasm_code,
            arg: Vec::<u8>::new(),
        };

        let result = management::InstallCode::perform_with_payment(
            Principal::management_canister(),
            (install_args,),
            10_000_000,
        )
        .await;

        if result.is_err() {
            self.status = NodeStatus::Error(NodeError::Initialize(format!(
                "Failed to initialize node {}",
                canister_id
            )));

            return false;
        }

        let args = InitCanisterManagerParam {
            args: Some(InstallArgs {
                all_nodes: self.canister.all_nodes().into_iter().cloned().collect(),
            }),
        };

        let result = ic::call::<_, (), _>(canister_id, "init_canister_manager", (args,)).await;

        if result.is_err() {
            self.status = NodeStatus::Error(NodeError::Initialize(format!(
                "Failed to initialize node {}",
                canister_id
            )));

            return false;
        }

        true
    }

    fn delete_node(&mut self) -> () {
        // todo!()
        // https://github.com/open-ic/open-storage/blob/main/backend/libraries/utils/src/canister/delete.rs
    }

    async fn migrate_to_node(&mut self, canister_id: Principal, data: Vec<(String, Data)>) -> bool {
        #[derive(CandidType, Deserialize)]
        struct Response {
            result: bool,
        }

        let call_migrate = |args: MigrateArgs| async {
            ic::call::<_, (), _>(
                canister_id,
                "handle_event",
                (CanisterManagerEvent::Migrate(args),),
            )
            .await
            .map(|_| true)
            .map_err(|e| e.1)
        };

        let encode_data_chunk = |data_chunk: DataChunk<Data>| -> Result<MigrateArgs, String> {
            data_chunk.encode().map(|data| MigrateArgs { data })
        };

        for data_chunk in data.chunks(100) {
            let result = match encode_data_chunk(DataChunk::new(data_chunk.to_vec())) {
                Ok(args) => call_migrate(args).await,
                Err(error) => Err(error),
            };

            match result {
                Ok(response) => {
                    if !response {
                        self.status = NodeStatus::Error(NodeError::Migration(format!(
                            "Failed to migrate data to node {}",
                            canister_id
                        )));
                        return false;
                    }
                }
                Err(error) => {
                    self.status = NodeStatus::Error(NodeError::Migration(error));
                    return false;
                }
            }
        }

        true
    }

    fn handle_migrate(&mut self, args: MigrateArgs) -> bool {
        match DataChunk::<Data>::decode(&args.data) {
            Ok(data_chunk) => {
                let data_chunk = data_chunk.data;
                for (key, value) in data_chunk {
                    self.canister.insert_data(key, value);
                }
                true
            }
            Err(e) => {
                self.status = NodeStatus::Error(NodeError::Migration(
                    "Failed to handle migrate data to node".to_string(),
                ));
                false
            }
        }
    }

    pub async fn lifecycle_handle_event(&mut self, event: CanisterManagerEvent) -> () {
        match event {
            CanisterManagerEvent::NodeCreated(node_id) => {
                if node_id != self.canister.id {
                    self.canister.add_node(node_id);
                    self.migrate_data(node_id).await;
                    self.broadcast_event(CanisterManagerEvent::NodeCreated(node_id))
                        .await;
                }
            }
            CanisterManagerEvent::NodeDeleted(node_id) => {
                if node_id != self.canister.id {
                    self.canister.remove_node(&node_id);
                    self.migrate_data(node_id).await;
                    self.broadcast_event(CanisterManagerEvent::NodeDeleted(node_id))
                        .await;
                }
            }
            CanisterManagerEvent::Migrate(migrate_args) => {
                self.handle_migrate(migrate_args);
            }
        }
    }

    async fn migrate_data(&mut self, node_id: Principal) -> bool {
        let data_for_migration = self.canister.get_data_to_migrate();
        let result = self.migrate_to_node(node_id, data_for_migration).await;
        result
    }

    async fn broadcast_event(&mut self, event: CanisterManagerEvent) -> () {
        // send a request to all nodes except node_id
        let all_canisters = self.canister.all_nodes();
        for &canister_id in all_canisters {
            if self.canister.id != canister_id {
                let result =
                    ic::call::<_, (), _>(canister_id, "handle_event", (event.clone(),)).await;
                if result.is_err() {
                    self.status = NodeStatus::Error(NodeError::Broadcast(format!(
                        "Failed to broadcast event {:?} to {}",
                        event, canister_id
                    )));
                    return;
                }
            }
        }
    }

    pub fn node_info(&self) -> NodeInfo {
        NodeInfo {
            all_nodes: self
                .canister
                .all_nodes()
                .iter()
                .map(|&principal| principal.to_string())
                .collect(),
            current_memory_usage: 0,
            status: self.status.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::node_manager::NodeStatus;

    use super::CanisterManager;
    use super::WasmInitArgs;
    use async_std::test as async_test;
    use ic_kit::mock_principals;
    use ic_kit::MockContext;
    use ic_kit::Principal;

    #[test]
    fn new_node() {
        let node_id = Principal::anonymous();
        let cm = CanisterManager::<String>::new(node_id);
        let node_info = cm.node_info();

        assert_eq!(node_info.all_nodes, vec![node_id.to_string()]);
    }

    #[async_test]
    async fn node_initialized_properly() {
        let node_id = mock_principals::alice();
        let previous_node = mock_principals::bob();

        MockContext::new()
            .with_caller(previous_node.clone())
            .with_id(node_id.clone())
            .with_constant_return_handler(())
            .inject();

        let mut cm = CanisterManager::<String>::new(node_id.clone());
        let all_nodes = vec![previous_node.clone()];

        cm.lifecyle_init_node(Some(all_nodes)).await;
        let node_info = cm.node_info();

        assert_eq!(
            node_info.all_nodes,
            vec![previous_node.to_string(), node_id.to_string()]
        );

        assert_eq!(cm.canister.prev_node_id, Some(previous_node));
        matches!(cm.get_status(), NodeStatus::Initialized);
    }

    #[test]
    fn node_wasm_initialized_properly() {
        let node_id = mock_principals::alice();
        let mut cm = CanisterManager::<String>::new(node_id.clone());

        assert!(cm.lifecycle_init_wasm(WasmInitArgs {
            position: 0,
            wasm_chunk: Vec::<u8>::default(),
        }));

        assert!(cm.lifecycle_init_wasm(WasmInitArgs {
            position: 1,
            wasm_chunk: Vec::<u8>::default(),
        }));

        assert!(cm.lifecycle_init_wasm(WasmInitArgs {
            position: 2,
            wasm_chunk: Vec::<u8>::default(),
        }));

        matches!(cm.get_status(), NodeStatus::Ready);
    }
}

// fn install_code(
//     management_canister: &Canister<ManagementCanister>,
//     canister_id: Principal,
//     waiter: &Delay,
//     new_node_info: NodeInfo,
// ) -> Result<Principal, ()> {
//     let result = management_canister
//         .install_code(&canister_id, &WASM_CODE)
//         .with_arg(InstallArgs {
//             node_info: Some(new_node_info),
//         })
//         .call_and_wait(waiter)
//         .await;

//     result.map_err(|_| ()).map(|_| canister_id)
// }

// fn create_new_node(
//     management_canister: &Canister<ManagementCanister>,
// ) -> Result<Principal, ()> {
//     match management_canister
//         .create_canister()
//         .with_controller(ic::id())
//         .as_provisional_create_with_amount(Some(1_000_000))
//         .build()
//     {
//         Ok(create_canister) => {
//             create_canister
//                 .map(|canister_id| canister_id)
//                 .call_and_wait(waiter)
//                 .await
//         }
//         Err(err) => {
//             println!("{:?}", err);
//             Err(())
//         }
//     };
// }

// fn scale_up() {
//     let agent = Agent::builder()
//         .with_url(URL)
//         .with_identity(create_identity())
//         .build()?;

//     let management_canister = ManagementCanister::create(&agent);

//     let waiter = garcon::Delay::builder()
//         .throttle(std::time::Duration::from_millis(500))
//         .timeout(std::time::Duration::from_secs(60 * 5))
//         .build();

//     let result = create_new_node(&management_canister, &waiter).and_then(|canister_id| {
//         install_code(
//             &management_canister,
//             canister_id,
//             &waiter,
//             NODE_INFO.with(|node_info| NodeInfo {
//                 index_node: node_info.borrow().index_node,
//                 all_nodes: node_info.borrow().all_nodes.clone().insert(canister_id),
//             }),
//         )
//     });

//     if Ok(canister_id) = result {
//         NODE_INFO.with(|node_info| {
//             let mut node_info = node_info.borrow_mut();
//             node_info.all_nodes.insert(canister_id);
//         });
//     }
// }
