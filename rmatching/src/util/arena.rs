use std::ops::{Index, IndexMut};

/// Simple index-based arena allocator backed by a `Vec` and a free list.
pub struct Arena<T> {
    items: Vec<T>,
    free_list: Vec<u32>,
    active: usize,
    touched: Vec<u32>,
    was_touched: Vec<bool>,
    is_active: Vec<bool>,
}

impl<T: Default> Arena<T> {
    pub fn new() -> Self {
        Arena {
            items: Vec::new(),
            free_list: Vec::new(),
            active: 0,
            touched: Vec::new(),
            was_touched: Vec::new(),
            is_active: Vec::new(),
        }
    }

    /// Allocate a slot, returning its index. Reuses freed slots when available.
    pub fn alloc(&mut self) -> u32 {
        let idx = if let Some(idx) = self.free_list.pop() {
            self.items[idx as usize] = T::default();
            idx
        } else {
            let idx = self.items.len() as u32;
            self.items.push(T::default());
            self.was_touched.push(false);
            self.is_active.push(false);
            idx
        };
        self.mark_allocated(idx);
        idx
    }

    /// Allocate a slot while resetting reused entries in-place.
    pub fn alloc_with_reset(&mut self, reset: impl FnOnce(&mut T)) -> u32 {
        let idx = if let Some(idx) = self.free_list.pop() {
            reset(&mut self.items[idx as usize]);
            idx
        } else {
            let idx = self.items.len() as u32;
            self.items.push(T::default());
            self.was_touched.push(false);
            self.is_active.push(false);
            idx
        };
        self.mark_allocated(idx);
        idx
    }

    /// Return a slot to the free list for reuse.
    pub fn free(&mut self, idx: u32) {
        self.is_active[idx as usize] = false;
        self.free_list.push(idx);
        self.active -= 1;
    }

    pub fn get(&self, idx: u32) -> &T {
        &self.items[idx as usize]
    }

    pub fn get_mut(&mut self, idx: u32) -> &mut T {
        &mut self.items[idx as usize]
    }

    /// Drop all items and reset the free list.
    pub fn clear(&mut self) {
        self.items.clear();
        self.free_list.clear();
        self.active = 0;
        self.touched.clear();
        self.was_touched.clear();
        self.is_active.clear();
    }

    /// Mark all slots used since the last recycle reusable while preserving storage.
    pub fn recycle_touched(&mut self, mut reset: impl FnMut(&mut T)) {
        while let Some(idx) = self.touched.pop() {
            reset(&mut self.items[idx as usize]);
            self.was_touched[idx as usize] = false;
            if self.is_active[idx as usize] {
                self.is_active[idx as usize] = false;
                self.free_list.push(idx);
                self.active -= 1;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.active
    }

    pub fn is_empty(&self) -> bool {
        self.active == 0
    }

    /// Borrow the underlying items slice (needed for read-only access while mutating other fields).
    pub fn items(&self) -> &[T] {
        &self.items
    }

    fn mark_allocated(&mut self, idx: u32) {
        if !self.was_touched[idx as usize] {
            self.was_touched[idx as usize] = true;
            self.touched.push(idx);
        }
        self.is_active[idx as usize] = true;
        self.active += 1;
    }
}

impl<T: Default> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Index<u32> for Arena<T> {
    type Output = T;
    fn index(&self, idx: u32) -> &T {
        &self.items[idx as usize]
    }
}

impl<T> IndexMut<u32> for Arena<T> {
    fn index_mut(&mut self, idx: u32) -> &mut T {
        &mut self.items[idx as usize]
    }
}
