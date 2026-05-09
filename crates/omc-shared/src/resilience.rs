use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use std::sync::OnceLock;

/// Process-relative monotonic clock in milliseconds.
static START: OnceLock<Instant> = OnceLock::new();

fn monotonic_millis() -> u64 {
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_millis() as u64
}

/// Circuit breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit breaker with exponential backoff retry.
///
/// Opens after `failure_threshold` consecutive failures, then
/// transitions to half-open after `reset_timeout` elapses.
pub struct CircuitBreaker {
    failure_threshold: u32,
    reset_timeout: Duration,
    state: Mutex<CircuitState>,
    failure_count: AtomicU32,
    last_failure_millis: AtomicU64,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            reset_timeout,
            state: Mutex::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            last_failure_millis: AtomicU64::new(0),
        }
    }

    /// Returns the current state, transitioning Open to HalfOpen if the
    /// reset timeout has elapsed.
    pub fn state(&self) -> CircuitState {
        let mut state = self.state.lock().unwrap();
        if *state == CircuitState::Open {
            let last = self.last_failure_millis.load(Ordering::Relaxed);
            let now = monotonic_millis();
            if now.saturating_sub(last) >= self.reset_timeout.as_millis() as u64 {
                *state = CircuitState::HalfOpen;
            }
        }
        *state
    }

    /// Record a successful call. Resets the failure count and closes the circuit.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        *state = CircuitState::Closed;
    }

    /// Record a failed call. Opens the circuit when the threshold is reached.
    pub fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.last_failure_millis
            .store(monotonic_millis(), Ordering::Relaxed);
        if count >= self.failure_threshold {
            let mut state = self.state.lock().unwrap();
            *state = CircuitState::Open;
        }
    }

    /// Check if a call is allowed given the current circuit state.
    /// Returns `true` if the call should proceed, `false` if blocked.
    pub fn allow_call(&self) -> bool {
        matches!(self.state(), CircuitState::Closed | CircuitState::HalfOpen)
    }
}

/// Retry a fallible async operation with exponential backoff.
///
/// Retries up to `max_retries` times with the given initial delay,
/// doubling the delay each attempt.
pub async fn retry_with_backoff<F, Fut, T, E>(
    mut op: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut delay = initial_delay;
    let mut last_err: Option<E> = None;

    for attempt in 0..=max_retries {
        match op().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                last_err = Some(e);
                if attempt < max_retries {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }

    Err(last_err.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_call());
    }

    #[test]
    fn test_failures_accumulate() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_threshold_opens_circuit() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.allow_call());
    }

    #[test]
    fn test_success_resets_failure_count() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(10));
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.failure_count.load(Ordering::Relaxed), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_open_to_halfopen_after_timeout() {
        // Use a very short timeout so the test is fast
        let cb = CircuitBreaker::new(1, Duration::from_millis(1));
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Sleep past the timeout
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.allow_call());
    }

    #[tokio::test]
    async fn test_retry_succeeds_on_first_try() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let result = retry_with_backoff(
            || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Ok::<_, String>("done")
                }
            },
            3,
            Duration::from_millis(1),
        )
        .await;

        assert_eq!(result.unwrap(), "done");
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let result = retry_with_backoff(
            || {
                let c = c.clone();
                async move {
                    let attempt = c.fetch_add(1, Ordering::Relaxed);
                    if attempt < 2 {
                        Err("not yet")
                    } else {
                        Ok("ok")
                    }
                }
            },
            3,
            Duration::from_millis(1),
        )
        .await;

        assert_eq!(result.unwrap(), "ok");
        assert_eq!(counter.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let result: Result<(), &str> = retry_with_backoff(
            || async { Err("always fails") },
            2,
            Duration::from_millis(1),
        )
        .await;

        assert_eq!(result.unwrap_err(), "always fails");
    }

    #[tokio::test]
    async fn test_retry_respects_max_retries() {
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let _: Result<(), ()> = retry_with_backoff(
            || {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::Relaxed);
                    Err(())
                }
            },
            4,
            Duration::from_millis(1),
        )
        .await;

        // 1 initial + 4 retries = 5 total attempts
        assert_eq!(counter.load(Ordering::Relaxed), 5);
    }
}
