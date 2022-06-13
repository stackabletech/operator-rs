#[cfg(test)]
mod tests {
    use stackable_operator_derive::Config;

    const DEFAULT_PORT: u16 = 11111;

    #[derive(Config)]
    pub struct FooConfig {
        #[config(default_value = "DEFAULT_PORT")]
        pub port: u16,
        pub vec: Vec<String>,
        #[config(default_impl = "test_default")]
        pub vec_impl: Vec<String>,
    }

    fn test_default() -> Vec<String> {
        vec!["barfoo".to_string()]
    }

    #[test]
    fn test_derive_config() {
        let mergable_config = MergableFooConfig {
            port: None,
            vec: Some(vec!["foo".to_string(), "bar".to_string()]),
            vec_impl: Some(vec!["foobar".to_string()]),
        };

        let config: FooConfig = mergable_config.into();

        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.vec_impl.as_ref(), vec!["foobar"]);
    }
}
