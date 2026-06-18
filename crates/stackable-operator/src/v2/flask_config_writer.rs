//! Writer for Flask App configurations (Python config files).
//!
//! Primitive types are escaped accordingly. Python expressions are written as-is;
//! invalid expressions produce invalid configuration files. Config overrides that do
//! not map to a known option are treated as plain expressions.

use std::{
    io::{self, Write},
    num::ParseIntError,
    str::{FromStr, ParseBoolError},
};

use snafu::{ResultExt, Snafu};

/// Errors which can occur when using this module
#[derive(Debug, Snafu)]
pub enum FlaskAppConfigWriterError {
    #[snafu(display("failed to convert '{value}' into a identifier"))]
    ConvertIdentifier { value: String },

    #[snafu(display("failed to convert '{value}' into a boolean literal"))]
    ConvertBoolLiteral {
        value: String,
        source: ParseBoolError,
    },

    #[snafu(display("failed to convert '{value}' into an integer literal"))]
    ConvertIntLiteral {
        value: String,
        source: ParseIntError,
    },

    #[snafu(display("failed to convert '{value}' into an ASCII string literal"))]
    ConvertStringLiteral { value: String },

    #[snafu(display("failed to convert '{value}' into a Python expression"))]
    ConvertExpression { value: String },

    #[snafu(display("Configuration cannot be written."))]
    WriteConfig { source: io::Error },
}

/// Mapping from configuration options to Python types.
pub trait FlaskAppConfigOptions {
    fn python_type(&self) -> PythonType;
}

/// All supported Python types
pub enum PythonType {
    /// Python identifier
    Identifier,
    /// Boolean literal
    BoolLiteral,
    /// Integer literal
    IntLiteral,
    /// ASCII string literal
    StringLiteral,
    /// Python expression
    Expression,
}

impl PythonType {
    /// Converts the given string to Python.
    fn convert_to_python(&self, value: &str) -> Result<String, FlaskAppConfigWriterError> {
        let convert = match self {
            Self::Identifier => Self::convert_to_python_identifier,
            Self::BoolLiteral => Self::convert_to_python_bool_literal,
            Self::IntLiteral => Self::convert_to_python_int_literal,
            Self::StringLiteral => Self::convert_to_python_string_literal,
            Self::Expression => Self::convert_to_python_expression,
        };

        convert(value)
    }

    fn convert_to_python_identifier(value: &str) -> Result<String, FlaskAppConfigWriterError> {
        if value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && value
                .chars()
                .next()
                .as_ref()
                .is_some_and(|c| !c.is_ascii_digit())
        {
            Ok(value.to_string())
        } else {
            ConvertIdentifierSnafu { value }.fail()
        }
    }

    fn convert_to_python_bool_literal(value: &str) -> Result<String, FlaskAppConfigWriterError> {
        value
            .parse::<bool>()
            .map(|b| if b { "True".into() } else { "False".into() })
            .context(ConvertBoolLiteralSnafu { value })
    }

    fn convert_to_python_int_literal(value: &str) -> Result<String, FlaskAppConfigWriterError> {
        value
            .parse::<i64>()
            .map(|i| i.to_string())
            .context(ConvertIntLiteralSnafu { value })
    }

    fn convert_to_python_string_literal(value: &str) -> Result<String, FlaskAppConfigWriterError> {
        if value.is_ascii() {
            Ok(format!("\"{}\"", value.escape_default()))
        } else {
            ConvertStringLiteralSnafu { value }.fail()
        }
    }

    fn convert_to_python_expression(value: &str) -> Result<String, FlaskAppConfigWriterError> {
        if value.trim().is_empty() {
            ConvertExpressionSnafu { value }.fail()
        } else {
            Ok(value.to_string())
        }
    }
}

/// Writes a configuration file according to the given `FlaskAppConfigOptions` type.
pub fn write<'a, O, P, W>(
    writer: &mut W,
    properties: P,
    imports: &[&str],
) -> Result<(), FlaskAppConfigWriterError>
where
    O: FlaskAppConfigOptions + FromStr,
    P: Iterator<Item = (&'a String, &'a String)>,
    W: Write,
{
    for import in imports {
        writeln!(writer, "{import}").context(WriteConfigSnafu)?;
    }

    writeln!(writer).context(WriteConfigSnafu)?;

    for (name, value) in properties {
        let variable = PythonType::Identifier.convert_to_python(name)?;

        // If an option cannot be mapped to a Python type then it is a config override and treated
        // as Python expression.
        let content = O::from_str(name)
            .map_or(PythonType::Expression, |option| option.python_type())
            .convert_to_python(value)?;

        writeln!(writer, "{variable} = {content}").context(WriteConfigSnafu)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        str::{FromStr, from_utf8},
    };

    use rstest::*;

    use super::{FlaskAppConfigOptions, FlaskAppConfigWriterError, PythonType, write};

    #[rstest]
    #[case::valid_identifiers_are_converted_to_python(
        PythonType::Identifier, &[
            ("_", "_"),
            ("a", "a"),
            ("A", "A"),
            ("__", "__"),
            ("_a", "_a"),
            ("_A", "_A"),
            ("_0", "_0"),
            ("SECRET_KEY", "SECRET_KEY"),
        ]
    )]
    #[case::valid_booleans_are_converted_to_python(
        PythonType::BoolLiteral, &[
            ("False", "false"),
            ("True", "true"),
        ]
    )]
    #[case::valid_integers_are_converted_to_python(
        PythonType::IntLiteral, &[
            ("-9223372036854775808", "-9223372036854775808"),
            ("0", "0"),
            ("9223372036854775807", "9223372036854775807"),
        ]
    )]
    #[case::valid_strings_are_converted_to_python(
        PythonType::StringLiteral, &[
            (r#""""#, ""),
            (r#"" ~""#, " ~"),
            (r#""\t\r\n\'\"\\""#, "\t\r\n'\"\\"),
        ]
    )]
    #[case::valid_expressions_are_converted_to_python(
        PythonType::Expression, &[
            ("os.environ[\"HOME\"]", "os.environ[\"HOME\"]"),
        ]
    )]
    fn valid_values_are_converted_to_python(
        #[case] python_type: PythonType,
        #[case] values: &[(&str, &str)],
    ) -> Result<(), FlaskAppConfigWriterError> {
        for (expected, input) in values {
            assert_eq!(*expected, python_type.convert_to_python(input)?);
        }

        Ok(())
    }

    #[rstest]
    #[case::invalid_identifiers_are_not_converted_to_python(
        PythonType::Identifier, &[
            "", "0", "-", "\n", "_-", "_\n",
        ]
    )]
    #[case::invalid_booleans_are_not_converted_to_python(
        PythonType::BoolLiteral, &[
            "", "False", "True", "0", "1",
        ]
    )]
    #[case::invalid_integers_are_not_converted_to_python(
        PythonType::IntLiteral, &[
            "", "a", "0x10", "inf",
        ]
    )]
    #[case::invalid_strings_are_not_converted_to_python(
        PythonType::StringLiteral, &[
            "ä", "❤"
        ]
    )]
    #[case::invalid_expressions_are_not_converted_to_python(
        PythonType::Expression, &[
            ""
        ]
    )]
    fn invalid_values_are_converted_to_python(
        #[case] python_type: PythonType,
        #[case] values: &[&str],
    ) {
        for input in values {
            assert!(python_type.convert_to_python(input).is_err());
        }
    }

    #[test]
    fn valid_options_are_written_into_a_configuration() {
        #[allow(clippy::enum_variant_names)]
        enum Options {
            BoolOption,
            IntOption,
            StringOption,
            ExpressionOption,
            _UnusedOption,
        }

        impl FromStr for Options {
            type Err = &'static str;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    "BOOL_OPTION" => Ok(Self::BoolOption),
                    "INT_OPTION" => Ok(Self::IntOption),
                    "STRING_OPTION" => Ok(Self::StringOption),
                    "EXPRESSION_OPTION" => Ok(Self::ExpressionOption),
                    _ => Err("unknown option"),
                }
            }
        }

        impl FlaskAppConfigOptions for Options {
            fn python_type(&self) -> PythonType {
                match self {
                    Self::BoolOption => PythonType::BoolLiteral,
                    Self::IntOption => PythonType::IntLiteral,
                    Self::StringOption => PythonType::StringLiteral,
                    Self::ExpressionOption | Self::_UnusedOption => PythonType::Expression,
                }
            }
        }

        let config: BTreeMap<_, _> = [
            ("BOOL_OPTION", "true"),
            ("INT_OPTION", "0"),
            ("STRING_OPTION", ""),
            ("EXPRESSION_OPTION", "{ \"key\": \"value\" }"),
            ("OVERRIDDEN_OPTION", "None"),
        ]
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .into();

        let imports = ["import module", "from module import member"];

        let mut config_file = Vec::new();
        write::<Options, _, _>(&mut config_file, config.iter(), &imports)
            .expect("writing the test configuration should succeed");

        assert_eq!(
            r#"import module
from module import member

BOOL_OPTION = True
EXPRESSION_OPTION = { "key": "value" }
INT_OPTION = 0
OVERRIDDEN_OPTION = None
STRING_OPTION = ""
"#,
            from_utf8(&config_file).expect("the Flask config writer only emits valid UTF-8")
        );
    }
}
