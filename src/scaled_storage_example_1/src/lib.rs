use ic_kit::{ic, macros::*};
use ref_thread_local::{ref_thread_local, RefThreadLocal};
use scaled_storage::node::NodeResult;
use scaled_storage::node_manager::{
    CanisterManager, CanisterManagerEvent, InitCanisterManagerParam, NodeInfo,
};

// this project should be renamed scaled_snippets
// thread_local! {
// static CANISTER_MANAGER: RefCell<CanisterManager<String>> = RefCell::new(CanisterManager::new(
//     ic::id()
//   ));
// }

// ref_thread_local! {
//     static managed CANISTER_MANAGER: CanisterManager<String> = CanisterManager::new(
//         ic::id()
//       );
// }

static mut CANISTER_MANAGER: Option<CanisterManager<String>> = None;

#[init]
fn init() {
    unsafe {
        CANISTER_MANAGER = Some(CanisterManager::new(ic::id()));
    }
}

#[update]
fn update_data() {
    unsafe {
        match CANISTER_MANAGER
            .as_mut()
            .unwrap()
            .canister
            .with_upsert_data_mut("key".to_string(), |data| {
                data.push_str("value");
            }) {
            NodeResult::NodeId(node_id) => {}
            NodeResult::Result(result) => result.unwrap(),
        }
    }
}

#[update]
async fn init_canister_manager(param: InitCanisterManagerParam) {
    unsafe {
        match param.args {
            Some(args) => CANISTER_MANAGER.as_mut().unwrap().lifecyle_init_node(
                Some(args.all_nodes),
                ic::id(),
                ic::caller(),
            ),
            None => {
                CANISTER_MANAGER
                    .as_mut()
                    .unwrap()
                    .lifecyle_init_node(None, ic::id(), ic::caller())
            }
        }
        .await
    }
}

#[heartbeat]
async fn heartbeat() {
    // CANISTER_MANAGER.borrow_mut().lifecyle_heartbeat_node().await;
    unsafe {
        CANISTER_MANAGER
            .as_mut()
            .unwrap()
            .lifecyle_heartbeat_node()
            .await;
    }
}

#[update]
async fn handle_event(event: CanisterManagerEvent) {
    unsafe {
        CANISTER_MANAGER
            .as_mut()
            .unwrap()
            .lifecycle_handle_event(event)
            .await
    }
}

#[query]
fn node_info() -> NodeInfo {
    unsafe { CANISTER_MANAGER.as_mut().unwrap().node_info() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ic_kit::{mock_principals, Canister, MockContext};
    use scaled_storage::node_manager::InstallArgs;

    #[test]
    fn initial_canister() {
        let node_id = mock_principals::alice();
        let caller = mock_principals::bob();

        MockContext::new()
            .with_caller(caller.clone())
            .with_id(node_id.clone())
            .inject();

        let node_info = node_info();
        assert_eq!(node_info.all_nodes, vec![node_id.to_string()]);
        // Canister::new()
    }

    #[async_std::test]
    async fn initialized_canister() {
        let node_id = mock_principals::alice();
        let previous_node = mock_principals::bob();

        MockContext::new()
            .with_caller(previous_node.clone())
            .with_id(node_id.clone())
            .with_constant_return_handler(())
            .inject();

        // let watcher = ctx.watch();

        init_canister_manager(InitCanisterManagerParam {
            args: Some(InstallArgs {
                all_nodes: vec![previous_node],
            }),
        })
        .await;

        let node_info = node_info();

        assert_eq!(
            node_info.all_nodes,
            vec![previous_node.to_string(), node_id.to_string()]
        );
    }
}
