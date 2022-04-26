use crate::node::Node;
use futures::executor::block_on;
use ic_cdk::export::{
    candid::{CandidType, Deserialize},
    Principal,
};

use ic_kit::candid::{ Decode, Encode};
use ic_kit::ic;
use ic_kit::interfaces::management::{self, CanisterSettings};
use ic_kit::interfaces::Method;
use serde::de::DeserializeOwned;

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum CanisterManagerEvent {
    NodeCreated(Principal),
    NodeDeleted(Principal),
    Migrate(MigrateArgs)
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct InstallArgs {
    pub all_nodes: Vec<Principal>,
    // index_node: Principal,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct InitCanisterManagerParam {
    pub args: Option<InstallArgs>,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum NodeStatus {
    Initialized,
    Ready,
    // Error(NodeError),
    ShutDown,
    Migrating,
    ScaleUp,
    ScaleDown,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum NodeError {
    Migration,
    ScaleUp,
    BubbleDown,
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

    fn encode(self) -> Result<Vec<u8>, ()> {
        // encode_args((self,)).map_err(|_| ())
        Encode!(&self).map_err(|_| ())
    }

    fn decode(data: &Vec<u8>) -> Result<Self, ()> {
        Decode!(data, DataChunk<Data>).map_err(|_| ())
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

#[derive(CandidType, Deserialize,Debug,Clone)]
pub struct MigrateArgs {
    #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

type Canister<Data> = Node<Principal, Data>;

pub struct CanisterManager<Data: Default + Clone> {
    status: NodeStatus,
    pub canister: Canister<Data>,
    // when this memory is reached, node is scaled up
    // max_memory: u64,
    // when this memory is reached, node is scaled down
    // min_memory: u64,
    // max_memory > reserve_memory + min_memory
    // reserve_memory: u64,
}

impl<'a, Data: Default + Clone + CandidType + DeserializeOwned> CanisterManager<Data> {
    pub fn new(node_id: Principal) -> Self {
        let mut new_canister: Node<Principal, Data> =
            Node::new(node_id.clone(), Default::default());

        //initial canister
        new_canister.add_node(node_id);

        Self {
            status: NodeStatus::Ready,
            canister: new_canister,
            // reserve_memory: 0,
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
        self.canister.prev_node_id.is_none() && matches!(self.status, NodeStatus::Ready)
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

    pub fn lifecyle_init_node(
        &mut self,
        all_nodes: Option<Vec<Principal>>,
        node_id: Principal,
        caller_node_id: Principal,
    ) -> () {
        self.status = NodeStatus::Ready;
        let mut new_canister: Node<Principal, Data> = Node::new(node_id, Default::default());

        if let Some(mut all_nodes) = all_nodes {
            all_nodes.push(node_id);
            for principal_id in all_nodes {
                new_canister.add_node(principal_id);
            }
        }

        new_canister.prev_node_id = Some(caller_node_id);
        self.canister = new_canister;

        block_on(self.broadcast_event(CanisterManagerEvent::NodeCreated(self.canister.id)));

        // self.node_info()
    }

    pub fn lifecyle_heartbeat_node(&mut self) -> () {
        if self.should_scale_up() {
            self.status = NodeStatus::ScaleUp;
            let create_node_result = block_on(self.create_node());

            match create_node_result {
                Some(new_node_id) => {
                    self.canister.add_node(new_node_id.clone());
                    block_on(self.initialize_node(new_node_id.clone()));
                    block_on(self.migrate_data(new_node_id));
                    self.canister.next_node_id = Some(new_node_id);
                    block_on(self.broadcast_event(CanisterManagerEvent::NodeCreated(new_node_id)));
                }
                None => {
                    self.status = NodeStatus::Ready;
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

    async fn initialize_node(&self, canister_id: Principal) -> bool {
        //vector of &Principal to Principal

        let args = InitCanisterManagerParam {
            args: Some(InstallArgs {
                all_nodes: self.canister.all_nodes().into_iter().cloned().collect(),
            }),
        };

        let result = ic::call::<_, (), _>(canister_id, "init_canister_manager", (args,)).await;

        result.is_ok()
    }

    fn delete_node(&mut self) -> () {
        // todo!()
        // https://github.com/open-ic/open-storage/blob/main/backend/libraries/utils/src/canister/delete.rs
    }

    async fn migrate_to_node(&self, canister_id: Principal, data: Vec<(String, Data)>) -> bool {
        #[derive(CandidType, Deserialize)]
        struct Response {
            result: bool,
        }

        let call_migrate = |args: MigrateArgs| async {
            ic::call::<_, (), _>(canister_id, "migrate", (args,))
                .await
                .map(|_| true)
                .map_err(|_| false)
        };

        let encode_data_chunk = |data_chunk: DataChunk<Data>| -> Result<MigrateArgs, ()> {
            data_chunk
                .encode()
                .map(|data| MigrateArgs { data })
                .map_err(|_| ())
        };

        for data_chunk in data.chunks(100) {
            let result = match encode_data_chunk(DataChunk::new(data_chunk.to_vec())) {
                Ok(args) => call_migrate(args).await,
                _ => Err(false),
            };

            if result.is_err() {
                break;
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
            Err(_) => false,
        }
    }

    pub fn lifecycle_handle_event(&mut self, event: CanisterManagerEvent) -> () {
        match event {
            CanisterManagerEvent::NodeCreated(node_id) => {
                if node_id != self.canister.id {
                    self.canister.add_node(node_id);
                    block_on(self.migrate_data(node_id));
                    block_on(self.broadcast_event(CanisterManagerEvent::NodeCreated(node_id)));
                }
            }
            CanisterManagerEvent::NodeDeleted(node_id) => {
                if node_id != self.canister.id {
                    self.canister.remove_node(&node_id);
                    block_on(self.migrate_data(node_id));
                    block_on(self.broadcast_event(CanisterManagerEvent::NodeDeleted(node_id)))
                }
            }
            _=>()
        }
    }

    async fn migrate_data(&mut self, node_id: Principal) {
        let data_for_migration = self.canister.get_data_to_migrate();
        self.status = NodeStatus::Migrating;
        self.migrate_to_node(node_id, data_for_migration).await;
        self.status = NodeStatus::Ready;
    }

    async fn broadcast_event(&self, event: CanisterManagerEvent) -> () {
        // send a request to all nodes
        let all_canisters = self.canister.all_nodes();
        for &canister_id in all_canisters {
            ic::call::<_, (), _>(canister_id, "handle_event", (event.clone(),)).await;
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
    use super::CanisterManager;
    use ic_kit::mock_principals;
    use ic_kit::Principal;

    #[test]
    fn new_node() {
        let node_id = Principal::anonymous();
        let cm = CanisterManager::<String>::new(node_id);
        let node_info = cm.node_info();

        assert_eq!(node_info.all_nodes, vec![node_id.to_string()]);
    }

    #[test]
    fn node_initialized_properly() {
        let node_id = mock_principals::alice();
        let previous_node = mock_principals::bob();

        let mut cm = CanisterManager::<String>::new(node_id.clone());
        let all_nodes = vec![previous_node.clone()];

        cm.lifecyle_init_node(Some(all_nodes), node_id, previous_node.clone());
        let node_info = cm.node_info();

        assert_eq!(
            node_info.all_nodes,
            vec![previous_node.to_string(), node_id.to_string()]
        );

        assert_eq!(cm.canister.prev_node_id, Some(previous_node));
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
