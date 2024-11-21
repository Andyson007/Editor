use std::sync::{Arc, RwLock};

use append_only_str::{slices::StrSlice, AppendOnlyStr};

use crate::table::InnerTable;

pub struct Client {
    buffer: Arc<RwLock<AppendOnlyStr>>,
    slice: Option<InnerTable<StrSlice>>,
}

impl Client {
    pub fn new(buffer: Arc<RwLock<AppendOnlyStr>>) -> Self {
        Self {
            buffer,
            slice: None,
        }
    }

    pub fn push_str(&mut self, to_push: &str) {
        self.buffer.write().unwrap().push_str(to_push);
        let slice = self
            .slice
            .as_mut()
            .expect("Can only call push_str in insert mode");
        let mut a = slice.write().unwrap();
        *a = self.buffer.read().unwrap().str_slice(a.start()..);
    }

    pub fn enter_insert(&mut self, inner_table: InnerTable<StrSlice>) {
        self.slice = Some(inner_table);
    }
}
