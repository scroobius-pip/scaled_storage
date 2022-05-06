# Scaled Storage

Scaled Storage is a multi-canister scaling solution for the Internet Computer.

## Features
1. Imported as a Rust library
2. Scales up or down depending on 
 developer defined behaviour.
3. “[Quine](https://en.wikipedia.org/wiki/Quine_(computing))” style canister replication. All canisters are functionally alike and use the same code.
4.  House keeping operations (migrations, scaling up and down) are abstracted away.
5. There isn’t a “primary”, “index” or “secondary” canister, any request can be taken from any canister.
6. Tries to reduce inter-canister calls.

## Run canister scaling test
The test uses a rust canister consuming the library. The canister has been configured to scale up after 10 keys have been added. It checks that all operations are successful, and can be done from any canister.

### Steps
1. Run `cargo build --bins`
2. Run `./deploy_dev.sh`
3. Run `./test_dev.sh [number_of_keys]`. 

If you for example run `./test_dev.sh 100` 10 canisters would be created.

## Usage
https://crates.io/crates/scaled_storage

MIT LICENSE
