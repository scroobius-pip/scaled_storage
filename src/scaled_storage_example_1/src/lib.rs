use candid::Principal;
use ic_kit::{ic, macros::*};
use scaled_storage::node_manager::{
    CanisterManager, CanisterManagerEvent, InitCanisterManagerParam, NodeInfo,
};
use std::cell::RefCell;
// this project should be renamed scaled_snippets
thread_local! {
    
    static CANISTER_MANAGER: RefCell<CanisterManager<String>> = RefCell::new(CanisterManager::new(
        ic::id()
      ));

}

#[update]
async fn init_canister_manager(param: InitCanisterManagerParam) {
    CANISTER_MANAGER.with(|canister_manager| {
        let mut canister_manager = canister_manager.borrow_mut();
        match param.args {
            Some(args) => {
                canister_manager.lifecyle_init_node(Some(args.all_nodes), ic::id(), ic::caller())
            }
            None => canister_manager.lifecyle_init_node(None, ic::id(), ic::caller()),
        }
    });
}

#[heartbeat]
fn heartbeat() {
    CANISTER_MANAGER
        .with(|canister_manager| canister_manager.borrow_mut().lifecyle_heartbeat_node())
}

#[update]
fn handle_event(event: CanisterManagerEvent) {
    CANISTER_MANAGER
        .with(|canister_manager| canister_manager.borrow_mut().lifecycle_handle_event(event))
}

#[query]
fn node_info() -> NodeInfo {
    CANISTER_MANAGER.with(|canister_manager| canister_manager.borrow().node_info())
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use ic_kit::{mock_principals, Canister, MockContext};
//     use scaled_storage::node_manager::InstallArgs;

//     #[test]
//     fn initial_canister() {
//         let node_id = mock_principals::alice();
//         let caller = mock_principals::bob();

//         MockContext::new()
//             .with_caller(caller.clone())
//             .with_id(node_id.clone())
//             .inject();

//         let node_info = node_info();
//         assert_eq!(node_info.all_nodes, vec![node_id.to_string()]);
//         // Canister::new()
//     }

//     #[test]
//     fn initialized_canister() {
//         let node_id = mock_principals::alice();
//         let previous_node = mock_principals::bob();

//         MockContext::new()
//             .with_caller(previous_node.clone())
//             .with_id(node_id.clone())
//             .inject();

//         init_canister_manager(InitCanisterManagerParam {
//             args: Some(InstallArgs {
//                 all_nodes: vec![previous_node],
//             }),
//         });

//         let node_info = node_info();

//         assert_eq!(
//             node_info.all_nodes,
//             vec![previous_node.to_string(), node_id.to_string()]
//         );
//     }
// }
