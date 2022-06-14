#[cfg(test)]
mod tests {
    use stackable_operator::config::config::Config;
    use stackable_operator::config::merge::Merge;

    const PORT: u16 = 22222;
    const DEFAULT_PORT: u16 = 11111;
    const FOO: &str = "foo";
    const BAR: &str = "bar";

    #[derive(Config)]
    pub struct FooConfigAtomicDefaultValue {
        #[config(default_value = "DEFAULT_PORT")]
        pub port: u16,
    }

    #[test]
    fn test_derive_config_atomic_default_value() {
        let config: FooConfigAtomicDefaultValue =
            MergableFooConfigAtomicDefaultValue { port: None }.into();
        assert_eq!(config.port, DEFAULT_PORT);

        let config: FooConfigAtomicDefaultValue =
            MergableFooConfigAtomicDefaultValue { port: Some(PORT) }.into();
        assert_eq!(config.port, PORT);
    }

    #[derive(Config)]
    pub struct FooConfigAtomicDefaultImpl {
        #[config(default_impl = "FooConfigAtomicDefaultImpl::default_impl")]
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
            MergableFooConfigAtomicDefaultImpl { vec: None }.into();
        // FOO from FooConfigAtomicDefaultImpl::default_impl
        assert_eq!(config.vec.as_ref(), vec![FOO]);

        let config: FooConfigAtomicDefaultImpl = MergableFooConfigAtomicDefaultImpl {
            vec: Some(vec![BAR.to_string()]),
        }
        .into();
        assert_eq!(config.vec.as_ref(), vec![BAR]);
    }

    #[derive(Config)]
    pub struct FooConfigAtomicNoDefault {
        pub vec: Vec<String>,
    }

    #[test]
    fn test_derive_config_atomic_no_default() {
        let config: FooConfigAtomicNoDefault =
            MergableFooConfigAtomicNoDefault { vec: None }.into();
        assert_eq!(config.vec, Vec::<String>::new());

        let config: FooConfigAtomicNoDefault = MergableFooConfigAtomicNoDefault {
            vec: Some(vec![BAR.to_string()]),
        }
        .into();
        assert_eq!(config.vec.as_ref(), vec![BAR]);
    }
}
