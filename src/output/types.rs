use crate::cli::args::CliOutputFormat;

/// The type of output a command prefers by default
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum OutputType {
    #[default]
    Text, // Human-readable table/text format
    Prompt, // LLM-optimized structured format
    Json,   // Machine-readable JSON
}

impl From<OutputType> for CliOutputFormat {
    fn from(output_type: OutputType) -> Self {
        match output_type {
            OutputType::Text => CliOutputFormat::Table,
            OutputType::Prompt => CliOutputFormat::Prompt,
            OutputType::Json => CliOutputFormat::Json,
        }
    }
}

impl From<CliOutputFormat> for OutputType {
    fn from(format: CliOutputFormat) -> Self {
        match format {
            CliOutputFormat::Table => OutputType::Text,
            CliOutputFormat::Md => OutputType::Text,
            CliOutputFormat::Yaml => OutputType::Json, // YAML is JSON-like structured output
            CliOutputFormat::Json => OutputType::Json,
            CliOutputFormat::Prompt => OutputType::Prompt,
        }
    }
}
