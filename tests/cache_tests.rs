use ghdash::cache::CacheStore;
use tempfile::TempDir;

#[test]
fn test_set_and_get() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    let data = vec!["hello".to_string(), "world".to_string()];
    store.set("test_key", &data).unwrap();

    let result: Option<Vec<String>> = store.get("test_key");
    assert_eq!(result, Some(data));
}

#[test]
fn test_get_missing_key_returns_none() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    let result: Option<Vec<String>> = store.get("nonexistent");
    assert_eq!(result, None);
}

#[test]
fn test_expired_entry_returns_none() {
    let dir = TempDir::new().unwrap();
    // TTL of 0 means everything is immediately expired
    let store = CacheStore::new(dir.path().to_path_buf(), 0);

    store.set("key", &42u32).unwrap();

    // Even though we just wrote it, TTL=0 means age (0) > ttl (0) is false,
    // but age=0 == ttl=0, so 0 > 0 is false — it should still be valid.
    // Let's use a sleep to ensure expiration.
    std::thread::sleep(std::time::Duration::from_millis(1100));

    let result: Option<u32> = store.get("key");
    assert_eq!(result, None);
}

#[test]
fn test_fresh_entry_with_short_ttl() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 60);

    store.set("key", &"value".to_string()).unwrap();

    let result: Option<String> = store.get("key");
    assert_eq!(result, Some("value".to_string()));
}

#[test]
fn test_invalidate_single_key() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    store.set("a", &1u32).unwrap();
    store.set("b", &2u32).unwrap();

    store.invalidate("a").unwrap();

    let a: Option<u32> = store.get("a");
    let b: Option<u32> = store.get("b");
    assert_eq!(a, None);
    assert_eq!(b, Some(2));
}

#[test]
fn test_invalidate_nonexistent_key_is_ok() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    // Should not error
    store.invalidate("nope").unwrap();
}

#[test]
fn test_invalidate_all() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    store.set("a", &1u32).unwrap();
    store.set("b", &2u32).unwrap();
    store.set("c", &3u32).unwrap();

    store.invalidate_all().unwrap();

    let a: Option<u32> = store.get("a");
    let b: Option<u32> = store.get("b");
    let c: Option<u32> = store.get("c");
    assert_eq!(a, None);
    assert_eq!(b, None);
    assert_eq!(c, None);
}

#[test]
fn test_invalidate_all_on_empty_dir() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    // Should not error on empty directory
    store.invalidate_all().unwrap();
}

#[test]
fn test_key_sanitization() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    // Keys with slashes should be sanitized
    store.set("org/repo", &"data".to_string()).unwrap();
    let result: Option<String> = store.get("org/repo");
    assert_eq!(result, Some("data".to_string()));
}

#[test]
fn test_corrupted_cache_file_returns_none() {
    let dir = TempDir::new().unwrap();
    let store = CacheStore::new(dir.path().to_path_buf(), 600);

    // Write garbage directly to the cache file
    let path = dir.path().join("bad_key.json");
    std::fs::write(&path, "not valid json!!!").unwrap();

    let result: Option<String> = store.get("bad_key");
    assert_eq!(result, None);
}

#[test]
fn test_creates_cache_dir_on_set() {
    let dir = TempDir::new().unwrap();
    let nested = dir.path().join("sub").join("dir");
    let store = CacheStore::new(nested.clone(), 600);

    assert!(!nested.exists());
    store.set("key", &"val".to_string()).unwrap();
    assert!(nested.exists());
}
