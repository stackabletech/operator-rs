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
        stackable_operator::kvp::Annotation::from_str($Input)
    }};
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn label_macro() {
        let label = label!("stackable.tech/vendor=Stackable").unwrap();
        assert_eq!(label.to_string(), "stackable.tech/vendor=Stackable");
    }

    #[test]
    fn annotation_macro() {
        let annotation = annotation!("stackable.tech/vendor=Hello Wörld!").unwrap();
        assert_eq!(annotation.to_string(), "stackable.tech/vendor=Hello Wörld!");
    }
}
