rm -r -f .dfx
dfx start --background
dfx deploy scaled_storage_example_1 --mode reinstall
./target/release/wasm_uploader ./target/wasm32-unknown-unknown/release/scaled_storage_example_1.wasm rrkah-fqaaa-aaaaa-aaaaq-cai http://localhost:8000