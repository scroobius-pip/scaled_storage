/// IC - A DHT solution for the internet computer
use anchorhash::AnchorHash;
// use anchorhash::AnchorHash::
use highway::HighwayBuildHasher;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;

pub struct Node<TId: Hash + Eq + Clone, Data: Default + Clone> {
    pub id: TId,
    data: HashMap<String, Data>,
    pub next_node_id: Option<TId>,
    pub prev_node_id: Option<TId>,
    // pub index_node_id: TId,
    all_nodes: Vec<TId>,
    hash: AnchorHash<String, TId, HighwayBuildHasher>,
}

impl<TId, Data> Node<TId, Data>
where
    TId: Eq + Hash + Clone,
    Data: Default + Clone,
{
    pub fn new(id: TId, all_nodes: HashSet<TId>) -> Self {
        Node {
            id,
            // index_node_id,
            all_nodes: all_nodes.clone().into_iter().collect(),
            hash: anchorhash::Builder::with_hasher(Default::default())
                .with_resources(all_nodes)
                .build(100),
            data: HashMap::new(),
            prev_node_id: None,
            next_node_id: None,
        }
    }

    pub fn with_data_mut<'a, F, R>(&mut self, key: String, action: F) -> NodeResult<TId, Option<R>>
    where
        F: FnOnce(&mut Data) -> R,
    {
        match self.node_id_from_data_key(&key) {
            Some(node_id) => {
                if node_id.clone() == self.id {
                    match self.data.get_mut(&key) {
                        Some(data) => NodeResult::Result(Some(action(data))),
                        None => NodeResult::Result(None),
                    }
                } else {
                    NodeResult::NodeId(node_id.clone())
                }
            }
            None => NodeResult::Result(None),
        }
    }

    pub fn insert_data(&mut self, key: String, data: Data) {
        self.data.insert(key, data);
    }

    /// same functionality as with_data_mut but keys not in node are added
    pub fn with_upsert_data_mut<'a, F, R>(
        &mut self,
        key: String,
        action: F,
    ) -> NodeResult<TId, Option<R>>
    where
        F: FnOnce(&mut Data) -> R,
    {
        match self.node_id_from_data_key(&key) {
            Some(node_id) => {
                if node_id.clone() == self.id {
                    match self.data.get_mut(&key) {
                        Some(data) => NodeResult::Result(Some(action(data))),
                        None => {
                            let data =
                                action(&mut self.data.entry(key).or_insert(Default::default()));
                            NodeResult::Result(Some(data))
                        }
                    }
                } else {
                    NodeResult::NodeId(node_id.clone())
                }
            }
            None => NodeResult::Result(None),
        }
    }

    fn node_id_from_data_key(&self, data_key: &String) -> Option<&TId> {
        self.hash.get_resource(data_key.clone())
    }

    pub fn add_node(&mut self, node_id: TId) -> bool {
        // check if node_id is already in hash
        match self.hash.resources().any(|id| id == &node_id) {
            true => false,
            false => {
                self.all_nodes.push(node_id.clone());
                self.hash.add_resource(node_id).is_ok()
            }
        }
    }

    pub fn remove_node(&mut self, node_id: &TId) -> bool {
        self.all_nodes.retain(|id| id != node_id);
        self.hash.remove_resource(node_id).is_ok()
    }

    fn get_keys_to_migrate(&self) -> Vec<&String> {
        self.data
            .keys()
            .filter(|key| -> bool {
                match self.node_id_from_data_key(key) {
                    Some(node_id) => *node_id != self.id,
                    None => false,
                }
            })
            .collect()
    }

    pub fn get_data_to_migrate(&self) -> Vec<(String, Data)> {
        let keys_to_migrate = self.get_keys_to_migrate();
        self.data
            .iter()
            .filter(|(key, _)| keys_to_migrate.contains(key))
            .map(|(key, data)| (key.clone(), data.clone()))
            .collect()
    }

    pub fn all_nodes(&self) -> Vec<&TId> {
        self.all_nodes.iter().collect()
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
    // fn handle_request(request: Request) -> Response {}
    // fn migrate_data_request()->Request{}
    // fn on_migrate_data_request(node_id: TId, data: Vec<Data>) -> Response {}

    // fn ping() {}
    // fn on_ping_request(){}
}

#[derive(Debug, PartialEq)]
pub enum NodeResult<TId, Data> {
    NodeId(TId),
    Result(Data),
}

impl<TId, Data> NodeResult<TId, Data> {
    pub fn or_forward_unwrap<O: FnOnce(TId) -> Data>(self, op: O) -> Data {
        match self {
            NodeResult::NodeId(node_id) => op(node_id),
            NodeResult::Result(data) => data,
        }
    }
}

enum Request {
    Migrate,
    Ping,
}

//
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_add_node() {
        let mut node_1 = Node::<_, String>::new("node_1".to_string(), HashSet::new());

        node_1.add_node("node_1".to_string());
        assert_eq!(
            node_1.node_id_from_data_key(&"data_key".to_string()),
            Some(&"node_1".to_string())
        );
    }

    #[test]
    fn can_remove_node() {
        let mut node_1 = Node::<_, String>::new("node_1".to_string(), HashSet::new());

        node_1.add_node("node_1".to_string());
        node_1.remove_node(&"node_1".to_string());
        assert_eq!(node_1.node_id_from_data_key(&"data_key".to_string()), None);
    }

    #[test]
    fn all_nodes() {
        let mut node_1 = Node::<_, String>::new("node_1".to_string(), HashSet::new());

        node_1.add_node("node_1".to_string());
        node_1.add_node("node_2".to_string());
        node_1.add_node("node_3".to_string());
        assert_eq!(node_1.all_nodes(), vec!["node_1", "node_2", "node_3"]);

        node_1.remove_node(&"node_2".to_string());
        assert_eq!(node_1.all_nodes(), vec!["node_1", "node_3"]);
    }

    #[test]
    fn data_is_distributed_on_different_nodes() {
        let mut index_node = Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

        index_node.add_node("index_node_id".to_string());
        index_node.add_node("node_1".to_string());

        let mut node_ids: Vec<String> = vec![];

        for id in 1..15 {
            let data_key = format!("data_key_{}", id);
            let node_id = index_node.node_id_from_data_key(&data_key).unwrap();
            node_ids.push(node_id.clone());
        }

        // "index_node_id" and "node_1" should exist in node_ids
        assert!(node_ids.contains(&"node_1".to_string()));
        assert!(node_ids.contains(&"index_node_id".to_string()));

        //node_1 should exist atleast 4 times in node_ids, Iterator::filter()
        let node_1_count = node_ids
            .iter()
            .filter(|&x| x == &"node_1".to_string())
            .count();
        assert!(node_1_count >= 4);

        //same with index_node_id
        let index_node_id_count = node_ids
            .iter()
            .filter(|&x| x == &"index_node_id".to_string())
            .count();
        assert!(index_node_id_count >= 4);
    }
    #[test]
    fn node_id_from_data_key_must_be_deterministic() {
        let mut node_ids: Vec<String> = vec![];

        for _ in 1..100 {
            let mut index_node =
                Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

            index_node.add_node("index_node_id".to_string());
            index_node.add_node("node_1".to_string());

            let data_key = "data_key".to_string();
            let node_id = index_node.node_id_from_data_key(&data_key).unwrap();
            node_ids.push(node_id.clone());
        }
        //all ids should be same
        assert_eq!(node_ids.iter().collect::<HashSet<_>>().len(), 1);
    }

    #[test]
    fn node_id_from_data_key_must_not_change_due_to_outoforder_node_insertions() {
        let mut node_ids_1: Vec<String> = vec![];
        let mut node_ids_2: Vec<String> = vec![];

        for id in 1..100 {
            let mut index_node =
                Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

            index_node.add_node("index_node_id".to_string());
            index_node.add_node("node_1".to_string());

            let data_key = &format!("data_key_{}", id);
            let node_id = index_node.node_id_from_data_key(&data_key).unwrap();
            node_ids_1.push(node_id.clone());
        }

        for id in 1..100 {
            let mut index_node_2 =
                Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

            index_node_2.add_node("node_1".to_string());
            index_node_2.add_node("index_node_id".to_string());

            let data_key = &format!("data_key_{}", id);
            let node_id = index_node_2.node_id_from_data_key(&data_key).unwrap();
            node_ids_2.push(node_id.clone());
        }

        assert_eq!(node_ids_1, node_ids_2);
    }

    #[test]
    fn with_data_mut_returns_result_none_when_key_maps_to_node_but_data_not_available() {
        let mut node_1 = Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

        node_1.add_node("index_node_id".to_string());

        let result = node_1.with_data_mut("data_key".to_string(), |data| {
            data.push_str("data");
            data.clone()
        });

        assert_eq!(result, NodeResult::Result(None));
    }

    #[test]
    fn with_data_mut_returns_result_when_key_maps_to_node_and_data_available() {
        let mut node_1 = Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

        node_1.add_node("index_node_id".to_string());
        node_1
            .data
            .insert("data_key".to_string(), "data".to_string());

        let result = node_1.with_data_mut("data_key".to_string(), |data| data.clone());

        assert_eq!(result, NodeResult::Result(Some("data".to_string())));
    }

    #[test]
    fn with_data_mut_returns_node_id_when_key_maps_to_different_node() {
        let mut node_1 = Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

        node_1.add_node("index_node_id".to_string());
        node_1.add_node("node_1".to_string());

        node_1
            .data
            .insert("data_key".to_string(), "data".to_string());
        node_1
            .data
            .insert("data_key_2".to_string(), "data_2".to_string());

        let result = node_1.with_data_mut("data_key_5".to_string(), |data| data.clone());

        assert_eq!(result, NodeResult::NodeId("node_1".to_string()));
    }

    #[test]
    fn get_keys_to_migrate_returns_added_nodes_ids() {
        let mut index_node = Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

        index_node.add_node("index_node_id".to_string());

        let mut all_keys = vec![];
        for id in 0..10 {
            let key = &format!("data_key_{}", id);
            all_keys.push(key.clone());
            index_node.with_upsert_data_mut(key.clone(), |data| {
                data.push_str("data");
            });
        }

        assert_eq!(index_node.data.len(), 10);
        // assert!(node_1.data.len() >= 4);
        index_node.add_node("node_1".to_string());

        let keys_to_migrate = index_node.get_keys_to_migrate();
        assert_eq!(
            keys_to_migrate.len() >= 4 && keys_to_migrate.len() < 8,
            true
        );

        assert!(keys_to_migrate
            .iter()
            .all(|key| index_node.node_id_from_data_key(key).unwrap() == &"node_1".to_string()));
    }

    #[test]
    fn get_keys_to_migrate_returns_deleted_nodes_ids() {
        let mut index_node = Node::<_, String>::new("index_node_id".to_string(), HashSet::new());

        let mut node_1 = Node::<_, String>::new("node_1".to_string(), HashSet::new());

        index_node.add_node("index_node_id".into());
        index_node.add_node("node_1".into());

        node_1.add_node("index_node_id".into());
        node_1.add_node("node_1".into());

        let mut all_keys = vec![];
        for id in 0..10 {
            let key = &format!("data_key_{}", id);
            all_keys.push(key.clone());
            index_node
                .with_upsert_data_mut(key.clone(), |data| {
                    data.push_str("data");
                })
                .or_forward_unwrap(|node_id| {
                    if node_id == "node_1".to_string() {
                        node_1.with_upsert_data_mut(key.clone(), |data| {
                            data.push_str("data");
                        });
                        Some(())
                    } else {
                        None
                    }
                });
        }
        //evenly distributed
        assert_eq!(index_node.data.len(), 5);
        assert_eq!(node_1.data.len(), 5);
        //let's see what happens when we delete one node
        node_1.remove_node(&"node_1".into());

        let keys_to_migrate = node_1.get_keys_to_migrate();

        assert!(keys_to_migrate
            .iter()
            .all(|key| node_1.node_id_from_data_key(key).unwrap() == &"index_node_id".to_string()));
    }
}
