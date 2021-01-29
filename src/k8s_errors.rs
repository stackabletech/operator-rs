use crate::error::Error;

use std::str::FromStr;

#[derive(Debug)]
pub enum StatusReason {
    /// AlreadyExists means the resource you are creating already exists.
    /// Details (optional):
    ///   "kind" string - the kind attribute of the conflicting resource
    ///   "id"   string - the identifier of the conflicting resource
    /// Status code 409
    AlreadyExists,
}

impl FromStr for StatusReason {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AlreadyExists" => Ok(StatusReason::AlreadyExists),
            _ => Err(()),
        }
    }
}

/// Returns a reason for an error if there is one.
/// The error may occur for any status reasons that are unknown.
pub fn reason_for_error<T>(result: &Result<T, Error>) -> Option<StatusReason> {
    match result {
        Err(Error::KubeError {
            source: kube::Error::Api(error),
        }) => match error.reason.parse() {
            Ok(reason) => Some(reason),
            _ => None,
        },
        _ => None,
    }
}

/// Returns true if the passed result indicates an API error with the reason `AlreadyExists`
pub fn is_already_exists<T>(result: &Result<T, Error>) -> bool {
    matches!(reason_for_error(result), Some(StatusReason::AlreadyExists))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use kube::error::ErrorResponse;

    #[test]
    fn test_reason_for_error() {
        let result = Ok(123);
        assert!(matches!(reason_for_error(&result), None));

        let result: Result<(), Error> = Err(Error::KubeError {
            source: kube::error::Error::RequestSend,
        });
        assert!(matches!(reason_for_error(&result), None));

        let result: Result<(), Error> = Err(Error::KubeError {
            source: kube::error::Error::Api(ErrorResponse {
                status: "".to_string(),
                message: "".to_string(),
                reason: "Foobar".to_string(),
                code: 0,
            }),
        });
        let result_2 = reason_for_error(&result);
        assert!(
            matches!(result_2, None),
            "Got [{:?}] expected [None]",
            result_2
        );

        let result: Result<(), Error> = Err(Error::KubeError {
            source: kube::error::Error::Api(ErrorResponse {
                status: "".to_string(),
                message: "".to_string(),
                reason: "AlreadyExists".to_string(),
                code: 0,
            }),
        });
        let result_2 = reason_for_error(&result);
        assert!(
            matches!(result_2, Some(StatusReason::AlreadyExists)),
            "Got [{:?}] expected [Some(StatusReason::AlreadyExists)]",
            result_2
        );
    }

    #[test]
    fn test_is_already_exists() {
        assert!(!is_already_exists(&Ok(123)));

        let result: Result<(), Error> = Err(Error::KubeError {
            source: kube::error::Error::Api(ErrorResponse {
                status: "".to_string(),
                message: "".to_string(),
                reason: "AlreadyExists".to_string(),
                code: 0,
            }),
        });
        assert!(is_already_exists(&result));
    }
}
