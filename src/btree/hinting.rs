use std::{borrow::Borrow, cell::Cell, marker::PhantomData};

use super::{
    borrow::DormantMutRef,
    map::*,
    node::{
        marker::{Immut, Leaf, LeafOrInternal},
        NodeRef,
    },
    search::SearchResult,
    cursor::Cursor,
};

#[derive(Debug, Clone, Copy)]
struct Hint;

impl<'a, K: 'a, V: 'a, Type> Copy for NodeRef<Hint, K, V, Type> {}
impl<'a, K: 'a, V: 'a, Type> Clone for NodeRef<Hint, K, V, Type> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, K: 'a, V: 'a> NodeRef<Immut<'a>, K, V, LeafOrInternal> {
    fn as_hint(self) -> NodeRef<Hint, K, V, LeafOrInternal> {
        unsafe { std::mem::transmute(self) }
    }
}
impl<K, V> NodeRef<Hint, K, V, LeafOrInternal> {
    unsafe fn as_ref<'a>(self) -> NodeRef<Immut<'a>, K, V, LeafOrInternal> {
        std::mem::transmute(self)
    }
}

pub struct BTreeWithHint<K, V> {
    map: BTreeMap<K, V>,
    hint: Cell<Option<NodeRef<Hint, K, V, LeafOrInternal>>>,
}

impl<K, V> Default for BTreeWithHint<K, V> {
    fn default() -> Self {
        Self {
            map: Default::default(),
            hint: Default::default(),
        }
    }
}

impl<K, V> BTreeWithHint<K, V> {
    unsafe fn get_hint(&self) -> Option<NodeRef<Immut, K, V, LeafOrInternal>> {
        unsafe { self.hint.get().map(|hint| hint.as_ref()) }
    }

    fn set_hint<'a>(&self, hint: NodeRef<Hint, K, V, LeafOrInternal>)
    where
        K: 'a,
        V: 'a,
    {
        self.hint.set(Some(hint))
    }
    fn clear_hint(&self) {
        self.hint.set(None);
    }
}

impl<K, V> BTreeWithHint<K, V> {
    fn search_tree<Q: ?Sized>(
        &self,
        key: &Q,
    ) -> Option<SearchResult<Immut, K, V, LeafOrInternal, Leaf>>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        use SearchResult::*;
        let mut root_node =
            // self.map.root.as_ref()?.reborrow();
            unsafe { self.get_hint() }.or_else(|| Some(self.map.root.as_ref()?.reborrow()))?;

        loop {
            match root_node.search_tree(key) {
                Found(handle) => {
                    self.set_hint(handle.into_node().as_hint());
                    return Some(Found(handle));
                }
                GoDown(handle) => {
                    self.set_hint(handle.into_node().forget_type().as_hint());
                    let node_len = handle.into_node().len();
                    if handle.idx() > 0 && handle.idx() < node_len {
                        return Some(GoDown(handle));
                    }
                    root_node = match root_node.ascend().ok() {
                        Some(parent) => parent.into_node().forget_type(),
                        None => return Some(GoDown(handle)),
                    };
                    root_node = match root_node.ascend().ok() {
                        Some(parent) => parent.into_node().forget_type(),
                        None => root_node,
                    };
                }
            }
        }
    }

    pub fn get_around<Q: ?Sized>(&self, key: &Q) -> (Option<(&K, &V)>, Option<(&K, &V)>)
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        use SearchResult::*;
        let (prev, next) = match self.search_tree(key) {
            Some(Found(kv_handle)) => {
                let prev = kv_handle.next_back_leaf_edge();
                let next = kv_handle.next_leaf_edge();
                (prev, next)
            }
            Some(GoDown(handle)) => (handle, handle),
            None => return (None, None),
        };
        (
            prev.next_back_kv().ok().map(|k| k.into_kv()),
            next.next_kv().ok().map(|k| k.into_kv()),
        )
    }

    pub fn previous<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        use SearchResult::*;
        let prev = match self.search_tree(key) {
            Some(Found(kv_handle)) => kv_handle.next_back_leaf_edge(),
            Some(GoDown(handle)) => handle,
            None => return None,
        };
        prev.next_back_kv().ok().map(|k| k.into_kv())
    }

    pub fn next<Q: ?Sized>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        use SearchResult::*;
        let next = match self.search_tree(key) {
            Some(Found(kv_handle)) => kv_handle.next_leaf_edge(),
            Some(GoDown(handle)) => handle,
            None => return None,
        };
        next.next_kv().ok().map(|k| k.into_kv())
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Ord,
    {
        use Entry::*;
        let new_hint;
        let output = match self.map.entry(key) {
            Occupied(mut entry) => {
                new_hint = Some(entry.handle.reborrow().into_node().as_hint());
                Some(entry.insert(value))
            }
            Vacant(entry) => {
                new_hint = entry
                    .handle
                    .as_ref()
                    .map(|h| h.reborrow().into_node().forget_type().as_hint());
                entry.insert(value);
                None
            }
        };
        if let Some(hint) = new_hint {
            unsafe {
                self.set_hint(hint);
            }
        }
        output
    }

    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        self.clear_hint();
        self.map.remove(key)
    }

    pub fn remove_and_return_around<Q: ?Sized>(&mut self, key: &Q) -> (Option<K>, Option<K>)
    where
        K: Borrow<Q> + Ord + Clone,
        Q: Ord,
    {
        let (map, dormant_map) = DormantMutRef::new(&mut self.map);
        let root = match map.root.as_mut() {
            Some(e) => e,
            None => panic!("could not find element to remove!"),
        };
        use SearchResult::*;
        let root_node = root.borrow_mut();
        match root_node.search_tree(key) {
            Found(handle) => {
                let prev = handle
                    .reborrow()
                    .next_back_leaf_edge()
                    .next_back_kv()
                    .ok()
                    .map(|k| k.into_kv().0)
                    .cloned();
                let next = handle
                    .reborrow()
                    .next_leaf_edge()
                    .next_kv()
                    .ok()
                    .map(|k| k.into_kv().0)
                    .cloned();
                OccupiedEntry {
                    handle,
                    dormant_map,
                    _marker: PhantomData,
                }
                .remove_entry();
                self.clear_hint();
                (prev, next)
            }
            GoDown(_) => panic!("could not find element to remove!"),
        }
    }

    pub fn iter(&self) -> Iter<'_, K, V> {
        self.map.iter()
    }

    pub fn cursor_before<Q: ?Sized>(&self, key: &Q) -> Option<Cursor<K, V>>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        use SearchResult::*;
        let prev = match self.search_tree(key) {
            Some(Found(kv_handle)) => {
                let prev = kv_handle.next_back_leaf_edge();
                prev
            }
            Some(GoDown(handle)) => handle,
            None => return None,
        };
        prev.next_back_kv().ok().map(|k| Cursor::new(k))
    }
    pub fn cursor_after<Q: ?Sized>(&self, key: &Q) -> Option<Cursor<K, V>>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        use SearchResult::*;
        let next = match self.search_tree(key) {
            Some(Found(kv_handle)) => {
                let next = kv_handle.next_leaf_edge();
                next
            }
            Some(GoDown(handle)) => handle,
            None => return None,
        };
        next.next_kv().ok().map(|k| Cursor::new(k))
    }
}

impl<K, V> IntoIterator for BTreeWithHint<K, V> {
    type Item = <BTreeMap<K, V> as IntoIterator>::Item;
    type IntoIter = <BTreeMap<K, V> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
