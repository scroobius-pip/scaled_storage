## Steps
### Initialize Canister Manager
```rust
use scaled_storage::node_manager::{
    CanisterManager, CanisterManagerEvent, InitCanisterManagerParam, NodeInfo, WasmInitArgs,
};

//Replace TYPE with your own data type
static mut CANISTER_MANAGER: Option<CanisterManager<TYPE>> = None;
#[init]
fn init(){
    unsafe {
        CANISTER_MANAGER = Some(CanisterManager::new(ic::id(), |size| size > 50));
        //replace closure with your own custom "should scale up" logic.
    }
}

```

### Add CanisterManager house-keeping methods

```rust


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

#[query]
fn node_info() -> NodeInfo {
    unsafe { CANISTER_MANAGER.as_mut().unwrap().node_info() }
}

```
### Update candid file
```text

type NodeError = variant {
    Migration: text;
    ScaleUp: text;
    Initialize: text;
    Broadcast: text;
};

type node_info_status = variant {
    Initialized;
    Ready;
    Error:NodeError;
    ShutDown;
    Migrating;
    ScaleUp;
    ScaleDown;
};


type node_info = record {
    all_nodes: vec text;
    prev_node_id: opt principal;
    next_node_id: opt principal;
    status: node_info_status;
    cycles_balance: nat64;
};


type install_args = record {
    all_nodes: vec text;
};

type init_canister_manager_param = record {
    args: opt install_args;
};

type migrate_args = record {
    data: blob;
};

type wasm_init_args = record {
    position: nat8;
    wasm_chunk: blob;
};

type canister_manager_event = variant {
 NodeCreated: text; 
 NodeDeleted: text;
 Migrate: migrate_args;
};


service: {
"init_canister_manager":(init_canister_manager_param)-> ();
"handle_event":(canister_manager_event)->();
"init_wasm":(wasm_init_args)->(bool);
 "node_info": () -> (node_info) query;
}

```

### Access your data
```rust
 unsafe {
     let canister_manager = &mut CANISTER_MANAGER.as_mut().unwrap().canister;

     let result = canister_manager.with_upsert_data_mut(key, |data| {
         *data = value;
         data.clone()
     });

     //result returns either a NodeResult::NodeId or NodeResult::Result

     match result {
         NodeResult::NodeId(node_id) => {
             //do something with node_id perhaps return it to the client
             //or forward the current request to the node_id like below
             CanisterManager::forward_request(node_id, "method_name", args)
         }
         NodeResult::Result(result) => {
             //do something with result (data.clone() from with_upsert_data_mut closure )
         }
     }
 }
 ```

 ### Once canister has been deployed, canister manager must be initialized with ss_uploader

 ```bash
 cargo install ss_uploader
 ```