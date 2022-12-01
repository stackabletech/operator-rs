use std::cmp;

use k8s_openapi::api::core::v1::Container;

use crate::{builder::ContainerBuilder, commons::product_image_selection::ResolvedProductImage};

use super::spec::{AutomaticContainerLogConfig, LogLevel};

const STACKABLE_CONFIG_DIR: &str = "/stackable/config";
const STACKABLE_LOG_DIR: &str = "/stackable/log";

pub const VECTOR_CONFIG_FILE: &str = "vector.toml";

pub fn capture_shell_output(
    log_dir: &str,
    container: &str,
    log_config: &AutomaticContainerLogConfig,
) -> String {
    let root_log_level = log_config.root_log_level();
    let console_log_level = cmp::max(
        root_log_level,
        log_config
            .console
            .as_ref()
            .and_then(|console| console.level_threshold)
            .unwrap_or_default(),
    );
    let file_log_level = cmp::max(
        root_log_level,
        log_config
            .file
            .as_ref()
            .and_then(|file| file.level_threshold)
            .unwrap_or_default(),
    );

    let log_file_dir = format!("{log_dir}/{container}");

    let stdout_redirect = match (
        console_log_level <= LogLevel::INFO,
        file_log_level <= LogLevel::INFO,
    ) {
        (true, true) => format!(" > >(tee {log_file_dir}/container.stdout.log)"),
        (true, false) => "".into(),
        (false, true) => format!(" > {log_file_dir}/container.stdout.log"),
        (false, false) => " > /dev/null".into(),
    };

    let stderr_redirect = match (
        console_log_level <= LogLevel::ERROR,
        file_log_level <= LogLevel::ERROR,
    ) {
        (true, true) => format!(" 2> >(tee {log_file_dir}/container.stderr.log >&2)"),
        (true, false) => "".into(),
        (false, true) => format!(" 2> {log_file_dir}/container.stderr.log"),
        (false, false) => " 2> /dev/null".into(),
    };

    let mut args = Vec::new();
    if file_log_level <= LogLevel::ERROR {
        args.push(format!("mkdir --parents {log_file_dir}"));
    }
    if stdout_redirect.is_empty() && stderr_redirect.is_empty() {
        args.push(":".into());
    } else {
        args.push(format!("exec{stdout_redirect}{stderr_redirect}"));
    }

    args.join(" && ")
}

pub fn create_log4j_config(
    log_dir: &str,
    log_file: &str,
    max_size_in_mb: i32,
    config: &AutomaticContainerLogConfig,
) -> String {
    let number_of_archived_log_files = 1;

    let loggers = config
        .loggers
        .iter()
        .filter(|(name, _)| name.as_str() != AutomaticContainerLogConfig::ROOT_LOGGER)
        .map(|(name, logger_config)| {
            format!(
                "log4j.logger.{name}={level}\n",
                name = name.escape_default(),
                level = logger_config.level.to_logback_literal(),
            )
        })
        .collect::<String>();

    format!(
        r#"log4j.rootLogger={root_log_level}, CONSOLE, FILE

log4j.appender.CONSOLE=org.apache.log4j.ConsoleAppender
log4j.appender.CONSOLE.Threshold={console_log_level_threshold}
log4j.appender.CONSOLE.layout=org.apache.log4j.PatternLayout
log4j.appender.CONSOLE.layout.ConversionPattern=%d{{ISO8601}} [myid:%X{{myid}}] - %-5p [%t:%C{{1}}@%L] - %m%n

log4j.appender.FILE=org.apache.log4j.RollingFileAppender
log4j.appender.FILE.Threshold={file_log_level_threshold}
log4j.appender.FILE.File={log_dir}/{log_file}
log4j.appender.FILE.MaxFileSize={max_log_file_size_in_mb}MB
log4j.appender.FILE.MaxBackupIndex={number_of_archived_log_files}
log4j.appender.FILE.layout=org.apache.log4j.xml.XMLLayout

{loggers}"#,
        max_log_file_size_in_mb = max_size_in_mb / (1 + number_of_archived_log_files),
        root_log_level = config.root_log_level().to_logback_literal(),
        console_log_level_threshold = config
            .console
            .as_ref()
            .and_then(|console| console.level_threshold)
            .unwrap_or_default()
            .to_logback_literal(),
        file_log_level_threshold = config
            .file
            .as_ref()
            .and_then(|file| file.level_threshold)
            .unwrap_or_default()
            .to_logback_literal(),
    )
}

pub fn create_logback_config(
    log_dir: &str,
    log_file: &str,
    max_size_in_mb: i32,
    config: &AutomaticContainerLogConfig,
) -> String {
    let number_of_archived_log_files = 1;

    let loggers = config
        .loggers
        .iter()
        .filter(|(name, _)| name.as_str() != AutomaticContainerLogConfig::ROOT_LOGGER)
        .map(|(name, logger_config)| {
            format!(
                "  <logger name=\"{name}\" level=\"{level}\" />\n",
                name = name.escape_default(),
                level = logger_config.level.to_logback_literal(),
            )
        })
        .collect::<String>();

    format!(
        r#"<configuration>
  <appender name="CONSOLE" class="ch.qos.logback.core.ConsoleAppender">
    <encoder>
      <pattern>%d{{ISO8601}} [myid:%X{{myid}}] - %-5p [%t:%C{{1}}@%L] - %m%n</pattern>
    </encoder>
    <filter class="ch.qos.logback.classic.filter.ThresholdFilter">
      <level>{console_log_level_threshold}</level>
    </filter>
  </appender>

  <appender name="FILE" class="ch.qos.logback.core.rolling.RollingFileAppender">
    <File>{log_dir}/{log_file}</File>
    <encoder class="ch.qos.logback.core.encoder.LayoutWrappingEncoder">
      <layout class="ch.qos.logback.classic.log4j.XMLLayout" />
    </encoder>
    <filter class="ch.qos.logback.classic.filter.ThresholdFilter">
      <level>{file_log_level_threshold}</level>
    </filter>
    <rollingPolicy class="ch.qos.logback.core.rolling.FixedWindowRollingPolicy">
      <minIndex>1</minIndex>
      <maxIndex>{number_of_archived_log_files}</maxIndex>
      <FileNamePattern>{log_dir}/{log_file}.%i</FileNamePattern>
    </rollingPolicy>
    <triggeringPolicy class="ch.qos.logback.core.rolling.SizeBasedTriggeringPolicy">
      <MaxFileSize>{max_log_file_size_in_mb}MB</MaxFileSize>
    </triggeringPolicy>
  </appender>

{loggers}
  <root level="{root_log_level}">
    <appender-ref ref="CONSOLE" />
    <appender-ref ref="FILE" />
  </root>
</configuration>
"#,
        max_log_file_size_in_mb = max_size_in_mb / (1 + number_of_archived_log_files),
        root_log_level = config.root_log_level().to_logback_literal(),
        console_log_level_threshold = config
            .console
            .as_ref()
            .and_then(|console| console.level_threshold)
            .unwrap_or_default()
            .to_logback_literal(),
        file_log_level_threshold = config
            .file
            .as_ref()
            .and_then(|file| file.level_threshold)
            .unwrap_or_default()
            .to_logback_literal(),
    )
}

pub fn create_vector_config(
    vector_aggregator_address: &str,
    config: Option<&AutomaticContainerLogConfig>,
) -> String {
    let vector_log_level = config
        .and_then(|config| config.file.as_ref())
        .and_then(|file| file.level_threshold)
        .unwrap_or_default();

    let vector_log_level_filter_expression = match vector_log_level {
        LogLevel::TRACE => "true",
        LogLevel::DEBUG => r#".level != "TRACE""#,
        LogLevel::INFO => r#"!includes(["TRACE", "DEBUG"], .metadata.level)"#,
        LogLevel::WARN => r#"!includes(["TRACE", "DEBUG", "INFO"], .metadata.level)"#,
        LogLevel::ERROR => r#"!includes(["TRACE", "DEBUG", "INFO", "WARN"], .metadata.level)"#,
        LogLevel::FATAL => "false",
        LogLevel::NONE => "false",
    };

    format!(
        r#"data_dir = "/stackable/vector/var"

[log_schema]
host_key = "pod"

[sources.vector]
type = "internal_logs"

[sources.files_stdout]
type = "file"
include = ["{STACKABLE_LOG_DIR}/*/*.stdout.log"]

[sources.files_stderr]
type = "file"
include = ["{STACKABLE_LOG_DIR}/*/*.stderr.log"]

[sources.files_log4j]
type = "file"
include = ["{STACKABLE_LOG_DIR}/*/*.log4j.xml"]

[sources.files_log4j.multiline]
mode = "halt_with"
start_pattern = "^<log4j:event"
condition_pattern = "</log4j:event>\r$"
timeout_ms = 10000

[transforms.processed_files_stdout]
inputs = ["files_stdout"]
type = "remap"
source = '''
.logger = "ROOT"
.level = "INFO"
'''

[transforms.processed_files_stderr]
inputs = ["files_stderr"]
type = "remap"
source = '''
.logger = "ROOT"
.level = "ERROR"
'''

[transforms.processed_files_log4j]
inputs = ["files_log4j"]
type = "remap"
source = '''
wrapped_xml_event = "<root xmlns:log4j=\"http://jakarta.apache.org/log4j/\">" + string!(.message) + "</root>"
parsed_event = parse_xml!(wrapped_xml_event).root.event
.timestamp = to_timestamp!(to_float!(parsed_event.@timestamp) / 1000)
.logger = parsed_event.@logger
.level = parsed_event.@level
.message = parsed_event.message
'''

[transforms.filtered_logs_vector]
inputs = ["vector"]
type = "filter"
condition = '{vector_log_level_filter_expression}'

[transforms.extended_logs_vector]
inputs = ["filtered_logs_vector"]
type = "remap"
source = '''
.container = "vector"
.level = .metadata.level
.logger = .metadata.module_path
if exists(.file) {{ .processed_file = del(.file) }}
del(.metadata)
del(.pid)
del(.source_type)
'''

[transforms.extended_logs_files]
inputs = ["processed_files_*"]
type = "remap"
source = '''
. |= parse_regex!(.file, r'^{STACKABLE_LOG_DIR}/(?P<container>.*?)/(?P<file>.*?)$')
del(.source_type)
'''

[sinks.aggregator]
inputs = ["extended_logs_*"]
type = "vector"
address = "{vector_aggregator_address}"
"#
    )
}

pub fn vector_container(
    image: &ResolvedProductImage,
    config_volume_name: &str,
    log_volume_name: &str,
) -> Container {
    // TODO Increase verbosity if root log level is lower than INFO.
    ContainerBuilder::new("vector")
        .unwrap()
        .image_from_product_image(image)
        .command(vec!["/stackable/vector/bin/vector".into()])
        .args(vec![
            "--config".into(),
            format!("{STACKABLE_CONFIG_DIR}/{VECTOR_CONFIG_FILE}"),
        ])
        .add_volume_mount(config_volume_name, STACKABLE_CONFIG_DIR)
        .add_volume_mount(log_volume_name, STACKABLE_LOG_DIR)
        .build()
}
