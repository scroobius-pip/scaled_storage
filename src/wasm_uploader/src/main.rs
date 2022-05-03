use candid::{CandidType, Decode, Deserialize, Encode};
use clap::Parser;
use garcon::Delay;
use ic_agent::{agent::{UpdateBuilder}, ic_types::Principal, identity::AnonymousIdentity, Agent,};
use read_byte_slice::{ByteSliceIter, FallibleStreamingIterator};
use std::fs::File;

#[derive(Parser, Debug)]
#[clap(author)]
struct Args {
    wasm_path: String,
    canister_id: String,
    url: String
}

#[derive(CandidType, Deserialize)]
pub struct WasmInitArgs {
    position: usize,
    wasm_chunk: Vec<u8>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let wasm_file = File::open(&args.wasm_path).unwrap();
    let mut byte_iterator = ByteSliceIter::new(wasm_file, 1024 * 1024);

    let agent = Agent::builder()
        .with_url(args.url)
        .with_identity(AnonymousIdentity)
        .build()
        .unwrap();
    
    agent.fetch_root_key().await;
    let mut update_builder = agent.update(
        &Principal::from_text(args.canister_id).unwrap(),
        "init_wasm",
    );

    let waiter = garcon::Delay::builder()
        .throttle(std::time::Duration::from_millis(500))
        .timeout(std::time::Duration::from_secs(60 * 5))
        .build();

    let result = init_wasm(
        WasmInitArgs {
            position: 0,
            wasm_chunk: byte_iterator.next().unwrap().unwrap().to_vec(),
        },
        &mut update_builder,
        &waiter,
    )
    .await;
    if !result {
        panic!("init_wasm failed (first_chunk)");
    }
    while let Ok(Some(chunk)) = byte_iterator.next() {
        // AsyncCallBuilder::new();
        let result = init_wasm(
            WasmInitArgs {
                position: 1,
                wasm_chunk: chunk.to_vec(),
            },
            &mut update_builder,
            &waiter,
        )
        .await;

        if !result {
            panic!("init_wasm failed");
        }
    }

    let result = init_wasm(
        WasmInitArgs {
            position: 2,
            wasm_chunk: vec![],
        },
        &mut update_builder,
        &waiter,
    )
    .await;

    if !result {
        panic!("init_wasm failed (last_chunk)");
    }
}

pub async fn init_wasm(
    args: WasmInitArgs,
    update_builder: &mut UpdateBuilder<'_>,
    waiter: &Delay,
) -> bool {
    let response = update_builder
        .with_arg(&Encode!(&args).unwrap())
        .call_and_wait(waiter.to_owned())
        .await
        .expect(format!("init_wasm failed {}", args.position).as_str());

    let result = Decode!(response.as_slice(), bool).unwrap();
    result
}
