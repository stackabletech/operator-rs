use serde::Serialize;

/// Converts a fallible lookup into an [`Option`], logging the error if there was one.
///
/// Used for individual values that are nice to have, so that failing to retrieve one of them
/// doesn't fail the whole component.
pub fn ok_or_warn<T, E: std::error::Error + 'static>(
    what: &str,
    result: Result<T, E>,
) -> Option<T> {
    result
        .inspect_err(|err| {
            tracing::warn!(
                error = err as &dyn std::error::Error,
                "failed to retrieve {what}, ignoring...",
            );
        })
        .ok()
}

/// Wraps errors returned by a component to present them consistently for serialization.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ComponentResult<T> {
    Ok(T),
    Err {
        #[serde(rename = "$error")]
        inner: ComponentError,
    },
}
impl<T> ComponentResult<T> {
    #[track_caller]
    pub fn report_from_result<E: std::error::Error + 'static>(
        component: &str,
        result: Result<T, E>,
    ) -> ComponentResult<T> {
        match result {
            Ok(x) => ComponentResult::Ok(x),
            Err(err) => {
                tracing::error!(
                    error = &err as &dyn std::error::Error,
                    "error reported by {component}, ignoring...",
                );
                ComponentResult::Err {
                    inner: ComponentError {
                        message: err.to_string(),
                        causes: std::iter::successors(err.source(), |err| err.source())
                            .map(|err| err.to_string())
                            .collect(),
                    },
                }
            }
        }
    }
}
#[derive(Debug, Serialize)]
pub struct ComponentError {
    message: String,
    causes: Vec<String>,
}
