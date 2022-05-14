use candid::{CandidType, Decode, Deserialize, Encode};
use clap::Parser;
use garcon::Delay;
use ic_agent::{
    agent::{QueryBuilder, UpdateBuilder},
    ic_types::Principal,
    identity::AnonymousIdentity,
    Agent,
};
use rand::seq::SliceRandom;
use random_string::generate;
use std::collections::HashSet;

#[derive(Parser, Debug)]
#[clap(author)]
struct Args {
    canister_id: String,
    url: String,
    size: usize,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let agent = Agent::builder()
        .with_url(args.url.clone())
        .with_identity(AnonymousIdentity)
        .build()
        .unwrap();

    println!("Parsed args");
    let _ = agent.fetch_root_key().await;

    let mut update_builder = agent.update(
        &Principal::from_text(args.canister_id).unwrap(),
        "update_data",
    );

    let waiter = garcon::Delay::builder()
        .throttle(std::time::Duration::from_millis(500))
        .timeout(std::time::Duration::from_secs(20))
        .build();

    println!("Created agent");

    let key_values = generate_key_value_pair(args.size.clone());

    println!("Generated key value pairs {:?}", key_values);

    let mut results: Vec<NodeResult> = Vec::new();
    let mut values: HashSet<String> = HashSet::new();

    // Able to send and retrieve all data intact
    for (key, value) in &key_values {
        println!("{} {}", key, value);
        let result = set(key, value, &mut update_builder, &waiter).await;
        values.insert(result.data.clone());
        results.push(result);
    }

    // All values are present in results
    assert_eq!(results.len(), key_values.len());
    assert!(key_values.iter().all(|(_, value)| values.contains(value)),);
    println!("All values are present in results");

    // Able to send from any canister
    let mut query_builders: Vec<QueryBuilder> = Vec::new();
    for canister_id in results
        .iter()
        .map(|result| result.from)
        .collect::<Vec<Principal>>()
    {
        let query_builder = agent.query(&canister_id, "get_data");
        query_builders.push(query_builder);
    }

    for (key, _) in key_values {
        //get random query builder

        let query_builder = query_builders.choose_mut(&mut rand::thread_rng()).unwrap();
        let result = get(key, query_builder).await;
        assert!(values.contains(&result.data), "{}", result.data);
    }

    println!("Queries can be received from any canister");
}

async fn set(
    key: &String,
    value: &String,
    update_builder: &mut UpdateBuilder<'_>,
    waiter: &Delay,
) -> NodeResult {
    let response = update_builder
        .with_arg(&Encode!(key, value).unwrap())
        .call_and_wait(waiter.to_owned())
        .await
        .unwrap();

    Decode!(response.as_slice(), NodeResult).unwrap()
}

async fn get(key: String, query_builder: &mut QueryBuilder<'_>) -> NodeResult {
    let response = query_builder
        .with_arg(&Encode!(&key).unwrap())
        .call()
        .await
        .unwrap();

    Decode!(response.as_slice(), NodeResult).unwrap()
}

fn generate_key_value_pair(size: usize) -> HashSet<(String, String)> {
    let charset = "1234567890abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut pairs = HashSet::new();

    for _ in 0..size {
        let key = generate(6, charset);
        let value = generate(6, charset);
        pairs.insert((key, value));
    }
    pairs

    //iterator of length size
    


}

#[derive(CandidType, Deserialize, Clone, Debug)]

struct NodeResult {
    data: String,
    from: Principal,
}
