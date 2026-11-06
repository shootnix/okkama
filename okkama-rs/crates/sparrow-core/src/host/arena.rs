//! Reusable typed storage with generational handles. No per-value ownership walk.
use crate::Error;

#[derive(Debug)]
struct Entry<T> {
    generation: u32,
    value: Option<T>,
    marked: bool,
    pinned: bool,
}

#[derive(Debug)]
pub(crate) struct Arena<T> {
    entries: Vec<Entry<T>>,
    free: Vec<usize>,
    pub allocations: usize,
    /// Unpinned values that survived the last sweep.
    pub live: usize,
    // Pinned entries form a prefix that collection never resets or sweeps.
    pinned: usize,
}
impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            free: Vec::new(),
            allocations: 0,
            live: 0,
            pinned: 0,
        }
    }
}
impl<T> Arena<T> {
    pub fn get(&self, handle: u64) -> Result<&T, Error> {
        self.entries
            .get(handle as u32 as usize)
            .filter(|e| e.generation == (handle >> 32) as u32)
            .and_then(|e| e.value.as_ref())
            .ok_or_else(|| Error("stale heap handle".into()))
    }
    pub fn get_mut(&mut self, handle: u64) -> Result<&mut T, Error> {
        self.entries
            .get_mut(handle as u32 as usize)
            .filter(|e| e.generation == (handle >> 32) as u32)
            .and_then(|e| e.value.as_mut())
            .ok_or_else(|| Error("stale heap handle".into()))
    }
    pub fn allocate(&mut self, value: T) -> Result<u64, Error> {
        self.allocations += 1;
        if let Some(index) = self.free.pop() {
            let entry = &mut self.entries[index];
            entry.generation += 1; // Exhausted generations are retired by sweep.
            entry.value = Some(value);
            return Ok((u64::from(entry.generation) << 32) | index as u64);
        }
        let index = u32::try_from(self.entries.len())
            .map_err(|_| Error("heap handle space exhausted".into()))?;
        self.entries.push(Entry {
            generation: 0,
            value: Some(value),
            marked: false,
            pinned: false,
        });
        Ok(u64::from(index))
    }
    /// Drop the value of a live handle now.
    pub fn release(&mut self, handle: u64) {
        let index = handle as u32 as usize;
        if let Some(e) = self.entries.get_mut(index).filter(|e| e.generation == (handle >> 32) as u32 && !e.pinned) {
            if e.value.take().is_some() && e.generation != u32::MAX {
                self.free.push(index);
            }
        }
    }
    pub fn begin_collection(&mut self) {
        for e in &mut self.entries[self.pinned..] {
            e.marked = false;
        }
    }
    pub fn mark(&mut self, handle: u64) -> Result<bool, Error> {
        self.get(handle)?;
        let e = &mut self.entries[handle as u32 as usize];
        let first = !e.marked;
        e.marked = true;
        Ok(first)
    }
    pub fn sweep(&mut self) {
        let mut live = 0;
        for (index, e) in self.entries.iter_mut().enumerate().skip(self.pinned) {
            if e.marked {
                live += usize::from(e.value.is_some());
            } else if !e.pinned && e.value.take().is_some() && e.generation != u32::MAX {
                self.free.push(index);
            }
        }
        self.live = live;
        self.allocations = 0;
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}
