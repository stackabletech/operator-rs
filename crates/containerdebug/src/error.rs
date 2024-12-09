use serde::Serialize;
use snafu::Snafu;

/// Wrapped version of the errors returned by [`sysinfo`], since they are bare [`str`]s.
#[derive(Debug, Snafu)]
#[snafu(display("{msg}"))]
pub struct SysinfoError {
    pub msg: &'static str,
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
                err.source();
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
