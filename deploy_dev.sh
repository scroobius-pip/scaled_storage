rm -r -f .dfx
dfx start --background
dfx_pid=$!
echo "yes" | dfx deploy scaled_storage_example_1 --mode reinstall
./target/release/wasm_uploader ./target/wasm32-unknown-unknown/release/scaled_storage_example_1.wasm rrkah-fqaaa-aaaaa-aaaaq-cai http://localhost:8000
# block until ctrl-c
sleep infinity
kill $dfx_pid
rm -r -f .dfx
