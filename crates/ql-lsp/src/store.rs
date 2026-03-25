use std::collections::HashMap;

use tokio::sync::RwLock;
use tower_lsp::lsp_types::Url;

#[derive(Debug, Default)]
pub(crate) struct DocumentStore {
    docs: RwLock<HashMap<Url, String>>,
}

impl DocumentStore {
    pub(crate) async fn insert(&self, uri: Url, source: String) {
        self.docs.write().await.insert(uri, source);
    }

    pub(crate) async fn get(&self, uri: &Url) -> Option<String> {
        self.docs.read().await.get(uri).cloned()
    }

    pub(crate) async fn remove(&self, uri: &Url) {
        self.docs.write().await.remove(uri);
    }
}
