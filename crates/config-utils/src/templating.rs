use std::{
    env,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};

use snafu::{OptionExt, ResultExt, Snafu};

use crate::{
    file_types::{FileType, KNOWN_FILE_TYPES},
    ENV_VAR_PATTERN_END, ENV_VAR_PATTERN_START, FILE_PATTERN_END, FILE_PATTERN_START,
};

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not read file {file_name:?}"))]
    ReadFile {
        source: std::io::Error,
        file_name: PathBuf,
    },

    #[snafu(display("Failed to get file extension from file {file_name:?}"))]
    GetFileExtension { file_name: PathBuf },

    #[snafu(display("Failed to convert file name {file_name:?} to string"))]
    ConvertFileNameToString { file_name: PathBuf },

    #[snafu(display("The extension {extension} is not known, can not determine file type. Please specify the file type manually."))]
    ExtensionUnkown { extension: String },

    #[snafu(display("Failed to create temporary file {tmp_file_name:?}"))]
    CreateTemporaryFile {
        source: std::io::Error,
        tmp_file_name: PathBuf,
    },

    #[snafu(display("Failed to read line from file {file_name:?}"))]
    ReadLine {
        source: std::io::Error,
        file_name: PathBuf,
    },

    #[snafu(display("Failed to write to temporary file {tmp_file_name:?}"))]
    WriteToTemporaryFile {
        source: std::io::Error,
        tmp_file_name: PathBuf,
    },

    #[snafu(display(
        "Failed to rename temporary file {tmp_file_name:?} to destination file {destination_file_name:?}"
    ))]
    RenameTemporaryFile {
        source: std::io::Error,
        tmp_file_name: PathBuf,
        destination_file_name: PathBuf,
    },

    #[snafu(display(
        "Could not find the end pattern {end_pattern:?} in expression {expression:?}"
    ))]
    FindEndPatten {
        end_pattern: String,
        expression: String,
    },

    #[snafu(display("Could not read file {file_name:?} for templating"))]
    ReadFileForTemplating {
        source: std::io::Error,
        file_name: PathBuf,
    },

    #[snafu(display("Could not read env var {env_var_name:?} for templating"))]
    ReadEnvVarForTemplating {
        source: std::env::VarError,
        env_var_name: String,
    },
}

type Result<T, E = Error> = std::result::Result<T, E>;

pub fn template(file_name: &PathBuf, file_type: Option<&FileType>, escape: bool) -> Result<()> {
    let file_type = match file_type {
        Some(file_type) => file_type,
        None => {
            let extension = file_name
                .extension()
                .context(GetFileExtensionSnafu { file_name })?
                .to_str()
                .context(GetFileExtensionSnafu { file_name })?;

            KNOWN_FILE_TYPES
                .get(extension)
                .context(ExtensionUnkownSnafu { extension })?
        }
    };

    let file = File::open(file_name).context(ReadFileSnafu { file_name })?;
    let buf_reader = BufReader::new(file);

    let tmp_file_name = PathBuf::from(format!(
        "{}.tmp_config_utils",
        file_name
            .to_str()
            .context(ConvertFileNameToStringSnafu { file_name })?
    ));
    let mut temp_file = File::create(&tmp_file_name).context(CreateTemporaryFileSnafu {
        tmp_file_name: tmp_file_name.clone(),
    })?;

    for line in buf_reader.lines() {
        let mut line = line.context(ReadLineSnafu { file_name })?;

        run_all_replacements_on_line(&mut line, file_type, escape)?;

        temp_file
            .write_all(line.as_bytes())
            .context(WriteToTemporaryFileSnafu {
                tmp_file_name: tmp_file_name.clone(),
            })?;
        temp_file
            .write_all(&[b'\n'])
            .context(WriteToTemporaryFileSnafu {
                tmp_file_name: tmp_file_name.clone(),
            })?;
    }

    fs::rename(&tmp_file_name, &file_name).context(RenameTemporaryFileSnafu {
        tmp_file_name,
        destination_file_name: file_name,
    })?;

    Ok(())
}

fn run_all_replacements_on_line(
    line: &mut String,
    file_type: &FileType,
    escape: bool,
) -> Result<()> {
    loop {
        let mut changed = false;
        changed |= replace_thingy_in_line(
            line,
            ENV_VAR_PATTERN_START,
            ENV_VAR_PATTERN_END,
            replacement_action_for_env_var,
            file_type,
            escape,
        )?;
        changed |= replace_thingy_in_line(
            line,
            FILE_PATTERN_START,
            FILE_PATTERN_END,
            replacement_action_for_file,
            file_type,
            escape,
        )?;

        if !changed {
            break;
        }
    }

    Ok(())
}

fn replacement_action_for_file(file_name: &str) -> Result<String> {
    let file_content =
        fs::read_to_string(file_name).context(ReadFileForTemplatingSnafu { file_name })?;
    let file_content = file_content.trim_end_matches('\n');

    Ok(file_content.to_owned())
}

fn replacement_action_for_env_var(env_var_name: &str) -> Result<String> {
    let env_var_content =
        env::var(env_var_name).context(ReadEnvVarForTemplatingSnafu { env_var_name })?;

    Ok(env_var_content)
}

/// * `line` is the current line [`String`] that should be templated.
/// * `start_pattern` must be the start pattern, e.g `${env:`.
/// * `end_pattern` must be the end pattern, `}` in the most cases.
/// * `replacement_action` must be a function that is called and get passed the [`&str`] content between the start and end
///   pattern. This can e.g. be the name of the env var or file name to read.
///
/// Returns wether the `line` was modified.
fn replace_thingy_in_line(
    line: &mut String,
    start_pattern: &str,
    end_pattern: &str,
    replacement_action: fn(&str) -> Result<String>,
    file_type: &FileType,
    escape: bool,
) -> Result<bool> {
    // We need to go back to forth to not destroy stuff while iterating.
    // Also this is needed to correctly handle nested cases.
    let matches = line
        .rmatch_indices(start_pattern)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        // Nothing to do
        return Ok(false);
    }

    for index in matches {
        debug_assert_eq!(&line[index..index + start_pattern.len()], start_pattern);
        let (parameter, _) = line[index + start_pattern.len()..]
            .split_once(end_pattern)
            .context(FindEndPattenSnafu {
                // FIXME: Truncate string to not bloat error message
                expression: &line[index..],
                end_pattern,
            })?;

        let mut new_content = replacement_action(parameter)?;
        if escape {
            new_content = file_type.escape(new_content);
        }

        line.replace_range(
            index..index + start_pattern.len() + parameter.len() + end_pattern.len(),
            &new_content,
        );
    }

    // We modified stuff
    Ok(true)
}
