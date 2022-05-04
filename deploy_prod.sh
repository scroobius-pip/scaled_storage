canister_id=$(dfx deploy scaled_storage_example_1 --mode reinstall --with-cycles 2000000000000 --network ic | grep -oh "\(\w*-\)*cai") 
echo "canister_id: $canister_id"
cargo run --bin wasm_uploader ./target/wasm32-unknown-unknown/release/scaled_storage_example_1.wasm $canister_id https://ic0.app