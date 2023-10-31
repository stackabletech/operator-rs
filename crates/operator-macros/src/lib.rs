#[macro_export]
macro_rules! kvp {
    ($Input:literal) => {{
        use std::str::FromStr;
        stackable_operator::kvp::KeyValuePair::from_str($Input)
    }};
}

#[macro_export]
macro_rules! label {
    ($Input:literal) => {
        $crate::kvp!($Input)
    };
}

#[macro_export]
macro_rules! annotation {
    ($Input:literal) => {
        $crate::kvp!($Input)
    };
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn macros() {
        let pair = kvp!("stackable.tech/vendor=Stackable").unwrap();
        assert_eq!(pair.to_string(), "stackable.tech/vendor=Stackable");
    }
}
