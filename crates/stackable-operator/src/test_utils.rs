/// Please use only in tests, as we have non-ideal error handling in case yaml_serde produced
/// non-utf8 output.
pub fn serialize_to_yaml_with_singleton_map<S>(input: &S) -> Result<String, yaml_serde::Error>
where
    S: serde::Serialize,
{
    use serde::ser::Error as _;

    let mut buffer = Vec::new();
    let mut serializer = yaml_serde::Serializer::new(&mut buffer);
    yaml_serde::with::singleton_map_recursive::serialize(input, &mut serializer)?;
    String::from_utf8(buffer).map_err(|err| {
        yaml_serde::Error::custom(format!(
            "For *some* reason, yaml_serde produced non-utf8 output: {err}"
        ))
    })
}
