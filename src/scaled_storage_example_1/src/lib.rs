use ic_kit::{ic, macros::*};
use scaled_storage::node::NodeResult;
use scaled_storage::node_manager::{
    CanisterManager, CanisterManagerEvent, InitCanisterManagerParam, NodeInfo, WasmInitArgs,
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
fn update_data(key: String, value: String) -> String {
    unsafe {
        match CANISTER_MANAGER
            .as_mut()
            .unwrap()
            .canister
            .with_upsert_data_mut(key.clone(), |data| {
                *data = value.clone();
                data.clone()
            }) {
            NodeResult::NodeId(node_id) => format!("{} in {}", key, node_id),
            NodeResult::Result(result) => result.unwrap(),
        }
    }
}

#[query]
fn get_data(key: String) -> String {
    unsafe {
        match CANISTER_MANAGER
            .as_mut()
            .unwrap()
            .canister
            .with_data_mut(key.clone(), |data| data.clone())
        {
            NodeResult::NodeId(node_id) => format!("{} in {}", key, node_id),
            NodeResult::Result(result) => result.unwrap(),
        }
    }
}

#[update]
async fn init_canister_manager(param: InitCanisterManagerParam) {
    unsafe {
        match param.args {
            Some(args) => CANISTER_MANAGER
                .as_mut()
                .unwrap()
                .lifecyle_init_node(Some(args.all_nodes), ic::id()),
            None => CANISTER_MANAGER
                .as_mut()
                .unwrap()
                .lifecyle_init_node(None, ic::id()),
        }
        .await
    }
}

#[update]
fn init_wasm(param: WasmInitArgs) -> bool {
    unsafe {
        CANISTER_MANAGER
            .as_mut()
            .unwrap()
            .lifecycle_init_wasm(param)
    }
}

#[heartbeat]
async fn heartbeat() {
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
                prev_node: Some(previous_node),
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
