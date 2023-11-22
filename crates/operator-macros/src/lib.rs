#[macro_export]
macro_rules! label {
    ($Input:literal) => {{
        use std::str::FromStr;
        stackable_operator::kvp::Label::from_str($Input)
    }};
}

#[macro_export]
macro_rules! annotation {
    ($Input:literal) => {{
        use std::str::FromStr;
        stackable_operator::kvp::KeyValuePair::from_str($Input)
    }};
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn macros() {
        let pair = label!("stackable.tech/vendor=Stackable").unwrap();
        assert_eq!(pair.to_string(), "stackable.tech/vendor=Stackable");
    }
}
