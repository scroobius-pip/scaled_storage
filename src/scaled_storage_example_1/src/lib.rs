use candid::{CandidType, Deserialize, Principal};
use ic_kit::{ic, macros::*};
use scaled_storage::node::NodeResult;
use scaled_storage::node_manager::{
    CanisterManager, CanisterManagerEvent, InitCanisterManagerParam, NodeInfo, WasmInitArgs,
};

static mut CANISTER_MANAGER: Option<CanisterManager<String>> = None;

#[init]
fn init() {
    unsafe {
        CANISTER_MANAGER = Some(CanisterManager::new(ic::id(), |size| size > 10));
    }
}

#[derive(CandidType, Deserialize, Clone, Debug)]
pub struct OperationResult {
    data: String,
    from: Principal,
}

#[update]
async fn update_data(key: String, value: String) -> OperationResult {
    unsafe {
        let canister_manager = &mut CANISTER_MANAGER.as_mut().unwrap().canister;

        match canister_manager.with_upsert_data_mut(key.clone(), |data| {
            *data = value.clone();
            data.clone()
        }) {
            NodeResult::NodeId(node_id) => {
                match CanisterManager::<String>::forward_request::<OperationResult, _, _>(
                    node_id,
                    "update_data",
                    (key, value),
                )
                .await
                {
                    Ok(result) => result,
                    Err(error) => OperationResult {
                        data: error,
                        from: node_id,
                    },
                }
            }
            NodeResult::Result(result) => OperationResult {
                data: result.unwrap_or_default(),
                from: canister_manager.id,
            },
        }
    }
}

#[query]
async fn get_data(key: String) -> OperationResult {
    unsafe {
        let canister = &mut CANISTER_MANAGER.as_mut().unwrap().canister;

        match canister.with_data_mut(key.clone(), |data| data.clone()) {
            NodeResult::NodeId(node_id) => {
                let result = CanisterManager::<String>::forward_request::<OperationResult, _, _>(
                    node_id,
                    "get_data",
                    (key,),
                )
                .await;
                match result {
                    Ok(result) => result,
                    Err(error) => OperationResult {
                        data: error,
                        from: node_id,
                    },
                }
            }
            NodeResult::Result(result) => OperationResult {
                data: result.unwrap_or_default(),
                from: canister.id,
            },
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
                .lifecyle_init_node(Some(args.all_nodes)),
            None => CANISTER_MANAGER.as_mut().unwrap().lifecyle_init_node(None),
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
    use ic_kit::{mock_principals, MockContext};
    use scaled_storage::node_manager::{InstallArgs, NodeStatus};

    #[test]
    fn initial_canister() {
        let node_id = mock_principals::alice();
        let caller = mock_principals::bob();

        MockContext::new()
            .with_caller(caller.clone())
            .with_id(node_id.clone())
            .inject();

        init();
        let node_info = node_info();
        assert_eq!(node_info.all_nodes, vec![node_id.to_string()]);
        matches!(node_info.status, NodeStatus::Initialized);
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

        init();
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

        matches!(node_info.status, NodeStatus::Initialized);
    }

    #[test]
    fn init_wasm_puts_node_on_ready() {
        let node_id = mock_principals::alice();
        let caller = mock_principals::bob();

        MockContext::new()
            .with_caller(caller.clone())
            .with_id(node_id.clone())
            .inject();

        init();

        init_wasm(WasmInitArgs {
            position: 0,
            wasm_chunk: Vec::<u8>::default(),
        });

        init_wasm(WasmInitArgs {
            position: 1,
            wasm_chunk: Vec::<u8>::default(),
        });

        init_wasm(WasmInitArgs {
            position: 2,
            wasm_chunk: Vec::<u8>::default(),
        });

        let node_info = node_info();

        matches!(node_info.status, NodeStatus::Ready);
    }
}
