use std::time::Duration;

use omc_shared::session_pool::{BoundedSessionPool, SessionPoolError};

#[test]
fn pool_reuses_rejects_overflow_closes_and_reaps() {
    let mut pool = BoundedSessionPool::new(1, Duration::from_millis(5));
    assert_eq!(
        *pool.get_or_try_insert_with("a", || Ok::<_, ()>(7)).unwrap(),
        7
    );
    assert_eq!(
        *pool.get_or_try_insert_with("a", || Ok::<_, ()>(8)).unwrap(),
        7
    );
    assert!(matches!(
        pool.get_or_try_insert_with("b", || Ok::<_, ()>(9)),
        Err(SessionPoolError::Capacity { limit: 1 })
    ));
    assert_eq!(pool.remove(&"a"), Some(7));
    pool.get_or_try_insert_with("b", || Ok::<_, ()>(9)).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    assert_eq!(pool.reap_idle(), 1);
    assert!(pool.is_empty());
}
