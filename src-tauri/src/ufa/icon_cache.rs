use lazy_static::lazy_static;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref CACHE: Mutex<VecDeque<(Arc<str>, Arc<str>)>> = Mutex::new(VecDeque::new());
}

/// Retrieves a cached icon by path, moving it to the front of the cache if found.
pub fn get(path: &str) -> Option<String> {
    let mut cache = CACHE.lock().unwrap();
    if let Some(pos) = cache.iter().position(|(p, _)| p.as_ref() == path) {
        let (_, value) = cache.remove(pos).expect("position exists");
        cache.push_front((Arc::from(path), value.clone()));
        Some(value.as_ref().to_string())
    } else {
        None
    }
}

/// Stores an icon in the cache, evicting the oldest entry if the cache is full.
pub fn put(path: &str, value: String) {
    let mut cache = CACHE.lock().unwrap();
    let path_arc = Arc::from(path);
    let value_arc = Arc::from(value.as_str());
    if let Some(pos) = cache.iter().position(|(p, _)| p.as_ref() == path) {
        cache.remove(pos);
    }
    cache.push_front((path_arc, value_arc));
    if cache.len() > super::types::ICON_CACHE_LIMIT {
        cache.pop_back();
    }
}
