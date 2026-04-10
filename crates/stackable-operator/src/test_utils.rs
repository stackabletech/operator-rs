/// Please use only in tests, as we have non-ideal error handling in case serde_yaml produced
/// non-utf8 output.
pub fn serialize_to_yaml_with_singleton_map<S>(input: &S) -> Result<String, serde_yaml::Error>
where
    S: serde::Serialize,
{
    use serde::ser::Error as _;

    let mut buffer = Vec::new();
    let mut serializer = serde_yaml::Serializer::new(&mut buffer);
    serde_yaml::with::singleton_map_recursive::serialize(input, &mut serializer)?;
    String::from_utf8(buffer).map_err(|err| {
        serde_yaml::Error::custom(format!(
            "For *some* reason, serde_yaml produced non-utf8 output: {err}"
        ))
    })
}
