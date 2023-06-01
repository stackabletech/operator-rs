#[macro_export]
macro_rules! unwrap_or_return {
    ( $e:expr, $r:expr ) => {
        match $e {
            Some(x) => x,
            None => return $r,
        }
    };
}
