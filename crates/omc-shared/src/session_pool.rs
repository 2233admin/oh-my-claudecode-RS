//! Small, transport-neutral bounded lifecycle store for process sessions.

use std::collections::{HashMap, hash_map::Entry as HashEntry};
use std::hash::Hash;
use std::time::{Duration, Instant};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionPoolError<E> {
    #[error("session capacity reached ({limit})")]
    Capacity { limit: usize },
    #[error("failed to open session")]
    Open(E),
}

struct Entry<V> {
    value: V,
    last_used: Instant,
}

/// In-process storage with reuse, a hard capacity, explicit close, and idle TTL.
pub struct BoundedSessionPool<K, V> {
    entries: HashMap<K, Entry<V>>,
    capacity: usize,
    idle_ttl: Duration,
}

impl<K: Eq + Hash, V> BoundedSessionPool<K, V> {
    pub fn new(capacity: usize, idle_ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            capacity,
            idle_ttl,
        }
    }

    pub fn get_or_try_insert_with<E>(
        &mut self,
        key: K,
        open: impl FnOnce() -> Result<V, E>,
    ) -> Result<&mut V, SessionPoolError<E>> {
        self.reap_idle();
        let at_capacity = self.entries.len() >= self.capacity;
        match self.entries.entry(key) {
            HashEntry::Occupied(mut occupied) => {
                occupied.get_mut().last_used = Instant::now();
                Ok(&mut occupied.into_mut().value)
            }
            HashEntry::Vacant(vacant) => {
                if at_capacity {
                    return Err(SessionPoolError::Capacity {
                        limit: self.capacity,
                    });
                }
                let value = open().map_err(SessionPoolError::Open)?;
                Ok(&mut vacant
                    .insert(Entry {
                        value,
                        last_used: Instant::now(),
                    })
                    .value)
            }
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.remove(key).map(|entry| entry.value)
    }

    pub fn reap_idle(&mut self) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, entry| entry.last_used.elapsed() < self.idle_ttl);
        before - self.entries.len()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
