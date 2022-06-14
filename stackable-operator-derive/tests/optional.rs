#[cfg(test)]
mod tests {
    use stackable_operator::config::merge::{Atomic, Merge};
    use stackable_operator::config::optional::Optional;

    const PORT: u16 = 22222;
    const DEFAULT_PORT: u16 = 11111;
    const FOO: &str = "foo";
    const BAR: &str = "bar";

    #[derive(Optional)]
    pub struct FooConfigAtomicDefaultValue {
        #[optional(default_value = "DEFAULT_PORT")]
        pub port: u16,
    }

    #[test]
    fn test_derive_config_atomic_default_value() {
        let config: FooConfigAtomicDefaultValue =
            OptionalFooConfigAtomicDefaultValue { port: None }.into();
        assert_eq!(config.port, DEFAULT_PORT);

        let config: FooConfigAtomicDefaultValue =
            OptionalFooConfigAtomicDefaultValue { port: Some(PORT) }.into();
        assert_eq!(config.port, PORT);
    }

    #[derive(Optional)]
    pub struct FooConfigAtomicDefaultImpl {
        #[optional(default_impl = "FooConfigAtomicDefaultImpl::default_impl")]
        pub vec: Vec<String>,
    }

    impl FooConfigAtomicDefaultImpl {
        fn default_impl() -> Vec<String> {
            vec![FOO.to_string()]
        }
    }

    #[test]
    fn test_derive_config_atomic_default_impl() {
        let config: FooConfigAtomicDefaultImpl =
            OptionalFooConfigAtomicDefaultImpl { vec: None }.into();
        // FOO from FooConfigAtomicDefaultImpl::default_impl
        assert_eq!(config.vec.as_ref(), vec![FOO]);

        let config: FooConfigAtomicDefaultImpl = OptionalFooConfigAtomicDefaultImpl {
            vec: Some(vec![BAR.to_string()]),
        }
        .into();
        assert_eq!(config.vec.as_ref(), vec![BAR]);
    }

    #[derive(Optional)]
    pub struct FooConfigAtomicNoDefault {
        pub vec: Vec<String>,
    }

    #[test]
    fn test_derive_config_atomic_no_default() {
        let config: FooConfigAtomicNoDefault =
            OptionalFooConfigAtomicNoDefault { vec: None }.into();
        assert_eq!(config.vec, Vec::<String>::new());

        let config: FooConfigAtomicNoDefault = OptionalFooConfigAtomicNoDefault {
            vec: Some(vec![BAR.to_string()]),
        }
        .into();
        assert_eq!(config.vec.as_ref(), vec![BAR]);
    }

    #[derive(Clone)]
    pub struct FooSubStruct {
        port: u16,
    }

    impl Default for FooSubStruct {
        fn default() -> Self {
            FooSubStruct { port: DEFAULT_PORT }
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub enum FooSubEnum {
        Complex(String),
    }

    impl Default for FooSubEnum {
        fn default() -> Self {
            FooSubEnum::Complex(BAR.to_string())
        }
    }

    impl Atomic for FooSubStruct {}
    impl Atomic for FooSubEnum {}

    #[derive(Optional)]
    pub struct FooConfigComplex {
        sub_struct: FooSubStruct,
        sub_enum: FooSubEnum,
    }

    #[test]
    fn test_derive_config_complex() {
        let config: FooConfigComplex = OptionalFooConfigComplex {
            sub_struct: Some(FooSubStruct { port: DEFAULT_PORT }),
            sub_enum: Some(FooSubEnum::Complex(FOO.to_string())),
        }
        .into();
        assert_eq!(config.sub_struct.port, DEFAULT_PORT);
        assert_eq!(config.sub_enum, FooSubEnum::Complex(FOO.to_string()));
    }

    #[derive(Optional)]
    pub struct FooConfigComplexDefaultImpl {
        #[optional(default_impl = "FooSubStruct::default")]
        sub_struct: FooSubStruct,
        sub_enum: FooSubEnum,
    }

    #[test]
    fn test_derive_config_complex_default_value() {
        let config: FooConfigComplexDefaultImpl = OptionalFooConfigComplexDefaultImpl {
            sub_struct: None,
            sub_enum: None,
        }
        .into();
        assert_eq!(config.sub_struct.port, DEFAULT_PORT);
        assert_eq!(config.sub_enum, FooSubEnum::Complex(BAR.to_string()));

        let config: FooConfigComplexDefaultImpl = OptionalFooConfigComplexDefaultImpl {
            sub_struct: Some(FooSubStruct { port: 22222 }),
            sub_enum: Some(FooSubEnum::Complex(FOO.to_string())),
        }
        .into();
        assert_eq!(config.sub_struct.port, 22222);
        assert_eq!(config.sub_enum, FooSubEnum::Complex(FOO.to_string()));
    }
}
