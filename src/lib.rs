#![allow(unused_unsafe)]
#![feature(
    rustc_attrs,
    dropck_eyepatch,
    extend_one,
    exclusive_range_pattern,
    core_intrinsics,
    allocator_api,
    new_uninit,
    maybe_uninit_slice,
    exact_size_is_empty,
    slice_ptr_get,
)]

mod btree;
pub use btree::hinting::BTreeWithHint as SweepTreeMap;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
