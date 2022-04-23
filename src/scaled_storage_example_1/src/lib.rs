use ic_cdk_macros::{heartbeat, init, query, update};
use scaled_storage::node_manager::{CanisterManager, InitCanisterManagerParam, NodeInfo};
use std::cell::RefCell;
// this project should be renamed scaled_snippets
thread_local! {
    static CANISTER_MANAGER: RefCell<CanisterManager<String>> = RefCell::new(CanisterManager::new(
        ic_cdk::id()
    ));
}

#[update]
fn init_canister_manager(param: InitCanisterManagerParam) {
    CANISTER_MANAGER.with(|canister_manager| {
        let mut canister_manager = canister_manager.borrow_mut();

        match param.args {
            Some(args) => canister_manager.lifecyle_init_node(
                Some(args.all_nodes),
                ic_cdk::id(),
                ic_cdk::caller(),
            ),
            None => canister_manager.lifecyle_init_node(None, ic_cdk::id(), ic_cdk::caller()),
        }
    });
}

#[heartbeat]
fn heartbeat() {
    CANISTER_MANAGER.with(|canister_manager| {
        canister_manager.borrow_mut().lifecyle_heartbeat_node();
    });
}

#[query]
fn node_info() -> NodeInfo {
    CANISTER_MANAGER.with(|canister_manager| {
        let canister_manager = canister_manager.borrow();
        canister_manager.node_info()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ic_kit::{mock_principals, Canister, MockContext};

    #[test]
    fn initial_canister_should_have_single_node_defined() {
        // Canister::new()
    }
}
