use std::fs::read_to_string;

use rustc_hash::FxHashMap;

use super::helpers::position_to_offset;

pub struct MemDocs {
    docs: FxHashMap<String, String>,
}

impl MemDocs {
    pub fn new() -> Self {
        MemDocs {
            docs: FxHashMap::default(),
        }
    }
    pub fn change(&mut self, range: lsp_types::Range, uri: String, text: String) {
        let doc = self.docs.get_mut(&uri.as_str().to_string()).unwrap();
        doc.replace_range(
            position_to_offset(doc, range.start)..position_to_offset(doc, range.end),
            &text,
        );
    }
    pub fn insert(&mut self, key: String, value: String) {
        self.docs.insert(key, value);
    }
    pub fn get(&self, key: &str) -> Option<&String> {
        self.docs.get(key)
    }
    pub fn get_file_content(&self, key: &str) -> Option<String> {
        let mem = self.get(key);
        if let Some(mem) = mem {
            return Some(mem.clone());
        }
        let re = read_to_string(key);
        if let Ok(re) = re {
            return Some(re);
        }
        None
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut String> {
        self.docs.get_mut(key)
    }
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.docs.remove(key)
    }
    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.docs.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Position;

    #[test]
    fn test_mem_docs() {
        let mut mem_docs = MemDocs::new();
        mem_docs.insert("test".to_string(), "test".to_string());
        assert_eq!(mem_docs.get("test"), Some(&"test".to_string()));
        assert_eq!(mem_docs.get_file_content("test"), Some("test".to_string()));
        assert_eq!(mem_docs.get_file_content("test2"), None);
        assert_eq!(mem_docs.remove("test"), Some("test".to_string()));
        assert_eq!(mem_docs.get("test"), None);
        mem_docs.insert("test".to_string(), "test".to_string());
        mem_docs.change(
            lsp_types::Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 3,
                },
            },
            "test".to_string(),
            "哒哒哒".to_string(),
        );
        assert_eq!(mem_docs.get("test"), Some(&"哒哒哒t".to_string()));
        mem_docs.change(
            lsp_types::Range {
                start: Position {
                    line: 0,
                    character: 1,
                },
                end: Position {
                    line: 0,
                    character: 2,
                },
            },
            "test".to_string(),
            "123".to_string(),
        );
        assert_eq!(mem_docs.get("test"), Some(&"哒123哒t".to_string()));
    }
}
