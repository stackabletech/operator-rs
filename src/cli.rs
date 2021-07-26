use clap::{App, Arg};

/// Retrieve a file path from CLI arguments that points to product-config file.
/// It is a temporary solution until we find out how to handle different CLI
/// arguments for different operators.
// TODO: write proper init method for all possible operator-rs arguments plus
//    operator specific arguments
pub fn product_config_path(name: &str, default_file_path: &str) -> String {
    let matches = App::new(name)
        .arg(
            Arg::with_name("product-config")
                .short("p")
                .long("product-config")
                .value_name("FILE")
                .help("Get path to a product-config file")
                .takes_value(true),
        )
        .get_matches();

    matches
        .value_of("product-config")
        .unwrap_or(default_file_path)
        .to_string()
}
