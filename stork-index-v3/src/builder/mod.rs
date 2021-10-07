use crate::{
    AnnotatedWord, Container, Entry, Excerpt, Index, PassthroughConfig, SearchResult,
    WordListSource,
};
use std::collections::HashMap;
use stork_config::Config;

mod fill_containers;
mod fill_intermediate_entries;
mod fill_stems;

mod annotated_words_from_string;
pub mod errors;
pub mod intermediate_entry;

use fill_containers::fill_containers;
use fill_intermediate_entries::fill_intermediate_entries;
use fill_stems::fill_stems;

use errors::{DocumentError, IndexGenerationError, WordListGenerationError};

use intermediate_entry::NormalizedEntry;

pub mod nudger;
use nudger::Nudger;

pub fn build(config: &Config) -> Result<Index, IndexGenerationError> {
    Nudger::from(config).print();

    let mut intermediate_entries: Vec<NormalizedEntry> = Vec::new();
    let mut document_errors: Vec<DocumentError> = Vec::new();
    let mut stems: HashMap<String, Vec<String>> = HashMap::new();
    let mut containers: HashMap<String, Container> = HashMap::new();

    fill_intermediate_entries(&config, &mut intermediate_entries, &mut document_errors)?;
    fill_stems(&intermediate_entries, &mut stems);
    fill_containers(&config, &intermediate_entries, &stems, &mut containers);

    let entries: Vec<Entry> = intermediate_entries.iter().map(Entry::from).collect();

    if entries.is_empty() {
        return Err(IndexGenerationError::NoValidFiles);
    }

    let passthrough_config = PassthroughConfig {
        url_prefix: config.input.url_prefix.clone(),
        title_boost: config.input.title_boost.clone(),
        excerpt_buffer: config.output.excerpt_buffer,
        excerpts_per_result: config.output.excerpts_per_result,
        displayed_results_count: config.output.displayed_results_count,
    };

    let index = Index {
        entries,
        containers,
        config: passthrough_config,
        errors: document_errors,
    };

    if config.input.break_on_file_error && !index.errors.is_empty() {
        return Err(IndexGenerationError::DocumentErrors(index));
    }

    Ok(index)
}

fn remove_surrounding_punctuation(input: &str) -> String {
    let mut chars: Vec<char> = input.chars().collect();

    while chars.first().unwrap_or(&'a').is_ascii_punctuation() {
        chars.remove(0);
    }

    while chars.last().unwrap_or(&'a').is_ascii_punctuation() {
        chars.pop();
    }

    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stork_config::File;
    use stork_config::*;

    fn generate_invalid_file_missing_selector() -> File {
        File {
            explicit_source: Some(DataSource::Contents("".to_string())),
            title: "Missing Selector".to_string(),
            filetype: Some(Filetype::HTML),
            html_selector_override: Some(".article".to_string()),
            ..Default::default()
        }
    }

    fn generate_invalid_file_empty_contents() -> File {
        File {
            explicit_source: Some(DataSource::Contents("".to_string())),
            title: "Empty Contents".to_string(),
            filetype: Some(Filetype::PlainText),
            ..Default::default()
        }
    }

    fn generate_valid_file() -> File {
        File {
            explicit_source: Some(DataSource::Contents("This is contents".to_string())),
            title: "Successful File".to_string(),
            filetype: Some(Filetype::PlainText),
            ..Default::default()
        }
    }

    #[test]
    fn missing_html_selector_fails_gracefully() {
        let config = Config {
            input: InputConfig {
                files: vec![
                    generate_invalid_file_missing_selector(),
                    generate_valid_file(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let build_results = build(&config).unwrap();

        assert_eq!(build_results.errors.len(), 1);

        let error_msg = build_results.errors.first().unwrap().to_string();

        assert!(
            error_msg.contains("HTML selector `.article` is not present in the file"),
            "{}",
            error_msg
        );
    }

    #[test]
    fn empty_contents_fails_gracefully() {
        let config = Config {
            input: InputConfig {
                files: vec![
                    generate_invalid_file_empty_contents(),
                    generate_valid_file(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let build_results = build(&config).unwrap();
        assert_eq!(build_results.errors.len(), 1);

        let error_msg = build_results.errors.first().unwrap().to_string();
        assert!(error_msg.contains("No words in word list"));
    }

    #[test]
    fn test_all_invalid_files_return_error() {
        let config = Config {
            input: InputConfig {
                files: vec![
                    generate_invalid_file_empty_contents(),
                    generate_invalid_file_missing_selector(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        let build_error = build(&config).unwrap_err();

        assert_eq!(build_error, IndexGenerationError::NoValidFiles);
    }

    #[test]
    fn test_failing_file_does_not_halt_indexing() {
        let config = Config {
            input: InputConfig {
                files: vec![
                    generate_invalid_file_missing_selector(),
                    generate_valid_file(),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(build(&config).unwrap().errors.len(), 1);
        assert_eq!(build(&config).unwrap().entries.len(), 1);
    }
}
