use super::node::{
    marker::{Immut, LeafOrInternal, KV},
    Handle, NodeRef,
};

pub struct Cursor<'a, K, V> {
    handle: Handle<NodeRef<Immut<'a>, K, V, LeafOrInternal>, KV>,
}

impl<'a, K: 'a, V: 'a> Cursor<'a, K, V> {
    pub(super) fn new(handle: Handle<NodeRef<Immut<'a>, K, V, LeafOrInternal>, KV>) -> Self {
        Self { handle }
    }

    pub fn kv(&self) -> (&'a K, &'a V) {
        self.handle.into_kv()
    }

    pub fn next(&mut self) -> bool {
        match self.handle.next_leaf_edge().next_kv().ok() {
            Some(k) => {
                self.handle = k;
                true
            }
            None => false,
        }
    }

    pub fn prev(&mut self) -> bool {
        match self.handle.next_back_leaf_edge().next_back_kv().ok() {
            Some(k) => {
                self.handle = k;
                true
            }
            None => false,
        }
    }
}
