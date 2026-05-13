/// Use [`std::sync::LazyLock`] to define a static "constant" from a string.
///
/// The string is converted into the given type with [`std::str::FromStr::from_str`].
///
/// # Examples
///
/// ```rust
/// constant!(DATA_VOLUME_NAME: VolumeName = "data");
/// constant!(pub CONFIG_VOLUME_NAME: VolumeName = "config");
/// ```
#[macro_export(local_inner_macros)]
macro_rules! constant {
    ($qualifier:vis $name:ident: $type:ident = $value:literal) => {
        $qualifier static $name: std::sync::LazyLock<$type> =
            std::sync::LazyLock::new(|| $type::from_str($value).expect("should be a valid $name"));
    };
}
