/// Use [`std::sync::LazyLock`] to define a static "constant" from a string.
///
/// The string is converted into the given type with [`std::str::FromStr::from_str`].
///
/// # Examples
///
/// ```rust
/// use std::str::FromStr;
///
/// use stackable_operator::constant;
/// use stackable_operator::v2::types::kubernetes::VolumeName;
/// use stackable_operator::v2::builder::pod::container::EnvVarName;
///
/// constant!(DATA_VOLUME_NAME: VolumeName = "data");
/// constant!(pub CONFIG_VOLUME_NAME: VolumeName = "config");
///
/// const CONFIG_OPTION_SECURITY_ENABLED: &str = "SECURITY_ENABLED";
/// constant!(ENV_VAR_NAME_SECURITY_ENABLED: EnvVarName = CONFIG_OPTION_SECURITY_ENABLED);
/// ```
#[macro_export(local_inner_macros)]
macro_rules! constant {
    ($qualifier:vis $name:ident: $type:ident = $value:expr) => {
        $qualifier static $name: std::sync::LazyLock<$type> =
            std::sync::LazyLock::new(|| $type::from_str($value).expect("should be a valid $name"));
    };
}
