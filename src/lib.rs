type VecSized = [u8; size_of::<Vec<u8>>()];

pub struct ErasedVec {
    header: VecSized,
    clear_fn: fn(&mut Self),
    drop_fn: fn(&mut Self),
}

impl ErasedVec {
    pub fn from<T>(vec: Vec<T>) -> Self {
        Self {
            header: unsafe { std::mem::transmute(vec) },
            clear_fn: |ev: &mut ErasedVec| ev.get_mut::<T>().clear(),
            drop_fn: |ev: &mut ErasedVec| unsafe { std::ptr::drop_in_place(ev.get_mut::<T>()) },
        }
    }

    pub fn get<T>(&self) -> &Vec<T> {
        unsafe { &*(&self.header as *const VecSized as *const Vec<T>) }
    }

    pub fn get_mut<T>(&mut self) -> &mut Vec<T> {
        unsafe { &mut *(&mut self.header as *mut VecSized as *mut Vec<T>) }
    }

    pub fn clear(&mut self) {
        (self.clear_fn)(self)
    }
}

impl Drop for ErasedVec {
    fn drop(&mut self) {
        (self.drop_fn)(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DropCounter(Arc<AtomicUsize>);

    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn new_counters(n: usize) -> (Vec<DropCounter>, Arc<AtomicUsize>) {
        let drops = Arc::new(AtomicUsize::new(0));
        let items = (0..n).map(|_| DropCounter(drops.clone())).collect();
        (items, drops)
    }

    #[test]
    fn from_preserves_elements() {
        let ev = ErasedVec::from(vec![1, 2, 3]);
        assert_eq!(ev.get::<i32>(), &[1, 2, 3]);
    }

    #[test]
    fn get_mut_allows_mutation() {
        let mut ev = ErasedVec::from(vec![1, 2, 3]);
        ev.get_mut::<i32>().push(4);
        assert_eq!(ev.get::<i32>(), &[1, 2, 3, 4]);
    }

    #[test]
    fn empty_vec_roundtrip() {
        let ev = ErasedVec::from(Vec::<u8>::new());
        assert!(ev.get::<u8>().is_empty());
        assert_eq!(ev.get::<u8>().capacity(), 0);
    }

    #[test]
    fn drop_drops_elements() {
        let (items, drops) = new_counters(2);
        {
            let _ev = ErasedVec::from(items);
            assert_eq!(drops.load(Ordering::SeqCst), 0);
        }
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn clear_drops_elements_and_keeps_capacity() {
        let (items, drops) = new_counters(3);
        let mut ev = ErasedVec::from(items);
        let cap_before = ev.get::<DropCounter>().capacity();

        ev.clear();

        assert_eq!(drops.load(Ordering::SeqCst), 3);
        assert_eq!(ev.get::<DropCounter>().len(), 0);
        assert_eq!(ev.get::<DropCounter>().capacity(), cap_before);
    }

    #[test]
    fn drop_after_clear_does_not_double_drop() {
        let (items, drops) = new_counters(2);
        let mut ev = ErasedVec::from(items);

        ev.clear();
        assert_eq!(drops.load(Ordering::SeqCst), 2);

        drop(ev);
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn clear_is_type_erased() {
        let (items, drops) = new_counters(2);
        let mut vecs = vec![
            ErasedVec::from(vec![1i32, 2, 3]),
            ErasedVec::from(items),
            ErasedVec::from(vec!["a".to_string(), "b".to_string()]),
        ];

        for ev in &mut vecs {
            ev.clear();
        }

        assert!(vecs[0].get::<i32>().is_empty());
        assert!(vecs[1].get::<DropCounter>().is_empty());
        assert!(vecs[2].get::<String>().is_empty());
        assert_eq!(drops.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn string_vec_roundtrip() {
        let mut ev = ErasedVec::from(vec!["a".to_string(), "b".to_string()]);
        ev.get_mut::<String>().push("c".into());
        assert_eq!(ev.get::<String>(), &["a", "b", "c"]);
    }
}
