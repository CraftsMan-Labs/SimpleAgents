//! Retry logic with exponential backoff.

use simple_agents_types::{
    config::RetryConfig,
    error::{Result, SimpleAgentsError},
};
use std::future::Future;

/// Execute an operation with retry logic.
///
/// This function will retry the operation according to the retry configuration,
/// applying exponential backoff between attempts.
///
/// # Arguments
/// - `config`: Retry configuration (max attempts, backoff, etc.)
/// - `error_is_retryable`: Function to determine if an error should be retried
/// - `operation`: The async operation to retry
///
/// # Returns
/// - `Ok(T)` if the operation succeeds
/// - `Err` with the last error if all retries are exhausted
///
/// # Example
/// ```no_run
/// use simple_agents_providers::retry::execute_with_retry;
/// use simple_agents_types::config::RetryConfig;
/// use simple_agents_types::error::{SimpleAgentsError, ProviderError};
///
/// # async fn example() -> Result<String, SimpleAgentsError> {
/// let config = RetryConfig::default();
///
/// execute_with_retry(
///     &config,
///     |e| matches!(e, SimpleAgentsError::Provider(pe) if pe.is_retryable()),
///     || async { Ok("success".to_string()) }
/// ).await
/// # }
/// ```
pub async fn execute_with_retry<F, Fut, T>(
    config: &RetryConfig,
    error_is_retryable: impl Fn(&SimpleAgentsError) -> bool,
    operation: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Check if this error is retryable
                if !error_is_retryable(&e) {
                    return Err(e);
                }

                // If this is the last attempt, return the error
                if attempt >= config.max_attempts - 1 {
                    last_error = Some(e);
                    break;
                }

                // Calculate backoff and sleep
                let backoff = config.calculate_backoff(attempt);
                tracing::debug!(
                    "Attempt {} failed, retrying after {:?}: {}",
                    attempt + 1,
                    backoff,
                    e
                );
                tokio::time::sleep(backoff).await;

                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use simple_agents_types::error::ProviderError;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let config = RetryConfig::default();

        let result = execute_with_retry(
            &config,
            |_| true,
            || async { Ok::<_, SimpleAgentsError>("success") },
        )
        .await;

        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let attempt_count = Arc::new(Mutex::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = execute_with_retry(
            &config,
            |_| true,
            || {
                let count = attempt_count_clone.clone();
                async move {
                    let mut attempts = count.lock().unwrap();
                    *attempts += 1;

                    if *attempts < 3 {
                        Err(SimpleAgentsError::Provider(ProviderError::Timeout(
                            Duration::from_secs(30),
                        )))
                    } else {
                        Ok("success")
                    }
                }
            },
        )
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(*attempt_count.lock().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let config = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let attempt_count = Arc::new(Mutex::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = execute_with_retry(
            &config,
            |_| true,
            || {
                let count = attempt_count_clone.clone();
                async move {
                    let mut attempts = count.lock().unwrap();
                    *attempts += 1;

                    Err::<String, _>(SimpleAgentsError::Provider(ProviderError::Timeout(
                        Duration::from_secs(30),
                    )))
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert_eq!(*attempt_count.lock().unwrap(), 3);
    }

    #[tokio::test]
    async fn test_non_retryable_error() {
        let config = RetryConfig::default();

        let attempt_count = Arc::new(Mutex::new(0));
        let attempt_count_clone = attempt_count.clone();

        let result = execute_with_retry(
            &config,
            |e| matches!(e, SimpleAgentsError::Provider(pe) if pe.is_retryable()),
            || {
                let count = attempt_count_clone.clone();
                async move {
                    let mut attempts = count.lock().unwrap();
                    *attempts += 1;

                    // Return a non-retryable error
                    Err::<String, _>(SimpleAgentsError::Validation(
                        simple_agents_types::error::ValidationError::Empty {
                            field: "test".to_string(),
                        },
                    ))
                }
            },
        )
        .await;

        assert!(result.is_err());
        // Should only attempt once for non-retryable errors
        assert_eq!(*attempt_count.lock().unwrap(), 1);
    }
}
