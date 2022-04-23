use std::str::FromStr;

// use ic_utils::call::AsyncCall;
// use ic_utils::interfaces::ManagementCanister;
// use ic_utils::Canister;
use crate::node::Node;
use ic_cdk::export::{
    candid::{CandidType, Deserialize},
    Principal,
};

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
    all_nodes: Vec<String>,
    current_memory_usage: u64,
    status: NodeStatus,
}

type Canister<Data> = Node<Principal, Data>;

pub struct CanisterManager<Data: Default + Clone> {
    status: NodeStatus,
    pub canister: Canister<Data>,
    /// when this memory is reached, node is scaled up
    max_memory: u64,
    /// when this memory is reached, node is scaled down
    min_memory: u64,
    // max_memory > reserve_memory + min_memory
    // reserve_memory: u64,
}

impl<'a, Data: Default + Clone> CanisterManager<Data> {
    pub fn new(node_id: Principal) -> Self {
        let mut new_canister: Node<Principal, Data> =
            Node::new(node_id.clone(), Default::default());

        //initial canister
        new_canister.add_node(node_id);

        Self {
            status: NodeStatus::Ready,
            canister: new_canister,
            max_memory: 0,
            min_memory: 0,
            // reserve_memory: 0,
        }
    }

    fn get_status(&self) -> &NodeStatus {
        &self.status
    }

    fn should_scale_up(&self) -> bool {
        let current_memory_usage: u64 = 0;
        current_memory_usage >= self.max_memory
            && self.canister.next_node_id.is_none()
            && matches!(self.status, NodeStatus::Ready)
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
    ) -> NodeInfo {
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

        self.broadcast_event(CanisterManagerEvent::NodeCreated(self.canister.id));
        self.node_info()
    }

    pub fn lifecyle_heartbeat_node(&mut self) -> () {
        if self.should_scale_up() {
            self.status = NodeStatus::ScaleUp;
            match self.create_node() {
                Some(new_node_id) => {
                    self.canister.add_node(new_node_id);
                    self.canister.next_node_id = Some(new_node_id);

                    let data_for_migration = self.canister.get_data_to_migrate();
                    self.status = NodeStatus::Migrating;
                    self.migrate_to_node(new_node_id, data_for_migration);

                    self.status = NodeStatus::Ready;
                    self.broadcast_event(CanisterManagerEvent::NodeCreated(new_node_id));
                }
                None => {
                    self.status = NodeStatus::Ready;
                }
            }
        } else if self.should_scale_down() {
            self.status = NodeStatus::ScaleDown;
            self.broadcast_event(CanisterManagerEvent::NodeDeleted(self.canister.id));
        }
    }

    fn create_node(&mut self) -> Option<Principal> {
        //create_new_canister
        //install code
        None
        // https://github.com/open-ic/open-storage/blob/main/backend/libraries/utils/src/canister/create.rs
    }

    fn delete_node(&self) -> () {
        todo!()
        // https://github.com/open-ic/open-storage/blob/main/backend/libraries/utils/src/canister/delete.rs
    }

    fn migrate_to_node(&self, canister_id: Principal, data: Vec<(String, Data)>) -> () {
        todo!()
    }

    fn handle_event(&mut self, event: CanisterManagerEvent) -> () {
        match event {
            CanisterManagerEvent::NodeCreated(node_id) => {
                if node_id != self.canister.id {
                    self.canister.add_node(node_id);
                    let data_for_migration = self.canister.get_data_to_migrate();
                    self.status = NodeStatus::Migrating;
                    self.migrate_to_node(node_id, data_for_migration);
                    self.status = NodeStatus::Ready;
                    self.broadcast_event(CanisterManagerEvent::NodeCreated(node_id));
                }
            }
            CanisterManagerEvent::NodeDeleted(node_id) => {
                if node_id != self.canister.id {
                    self.broadcast_event(CanisterManagerEvent::NodeDeleted(node_id));
                }
            }
        }
    }

    fn broadcast_event(&self, event: CanisterManagerEvent) -> () {
        // send a request to all nodes
        let all_nodes = self.canister.all_nodes();
        for node_id in all_nodes {}
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

#[derive(Clone, Debug, CandidType, Deserialize)]
enum CanisterManagerEvent {
    NodeCreated(Principal),
    NodeDeleted(Principal),
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
//     waiter: &Delay,
// ) -> Result<Principal, ()> {
//     match management_canister
//         .create_canister()
//         .with_controller(ic_cdk::id())
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
