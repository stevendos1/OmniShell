//! Memory store unit tests.

use super::*;

#[tokio::test]
async fn test_store_retrieve() {
    let s = InMemoryStore::new();
    s.store("ns1", "k1", "v1").await.unwrap();
    assert_eq!(s.retrieve("ns1", "k1").await.unwrap(), Some("v1".into()));
}

#[tokio::test]
async fn test_retrieve_missing() {
    let s = InMemoryStore::new();
    assert_eq!(s.retrieve("ns1", "missing").await.unwrap(), None);
}

#[tokio::test]
async fn test_list_keys() {
    let s = InMemoryStore::new();
    s.store("ns1", "a", "1").await.unwrap();
    s.store("ns1", "b", "2").await.unwrap();
    let mut keys = s.list_keys("ns1").await.unwrap();
    keys.sort();
    assert_eq!(keys, vec!["a", "b"]);
}

#[tokio::test]
async fn test_delete() {
    let s = InMemoryStore::new();
    s.store("ns1", "k", "v").await.unwrap();
    s.delete("ns1", "k").await.unwrap();
    assert_eq!(s.retrieve("ns1", "k").await.unwrap(), None);
}
