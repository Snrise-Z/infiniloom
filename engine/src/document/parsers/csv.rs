//! CSV document parser.
//!
//! Converts CSV content into a Document with a single table section.

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Parse CSV content into a Document containing a table.
pub fn parse(content: &str, _options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let mut doc = Document::new("", DocumentFormat::Csv);
    let mut lines = content.lines();

    // First non-empty line is headers
    let header_line = loop {
        match lines.next() {
            Some(line) if !line.trim().is_empty() => break line,
            Some(_) => continue,
            None => return Ok(doc),
        }
    };

    let delimiter = detect_delimiter(header_line);
    let headers: Vec<String> = split_csv_line(header_line, delimiter);
    let mut rows: Vec<Vec<String>> = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        rows.push(split_csv_line(line, delimiter));
    }

    let mut section = Section::root();
    section.content.push(ContentBlock::Table(Table {
        caption: None,
        headers,
        rows,
        alignments: Vec::new(),
    }));
    doc.sections.push(section);

    Ok(doc)
}

fn detect_delimiter(line: &str) -> char {
    let tab_count = line.chars().filter(|&c| c == '\t').count();
    let comma_count = line.chars().filter(|&c| c == ',').count();
    let semicolon_count = line.chars().filter(|&c| c == ';').count();
    let pipe_count = line.chars().filter(|&c| c == '|').count();

    let max = tab_count
        .max(comma_count)
        .max(semicolon_count)
        .max(pipe_count);
    if max == 0 {
        return ',';
    }
    if tab_count == max {
        '\t'
    } else if semicolon_count == max {
        ';'
    } else if pipe_count == max {
        '|'
    } else {
        ','
    }
}

fn split_csv_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            if in_quotes {
                // Check for escaped quote ("")
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                in_quotes = true;
            }
        } else if ch == delimiter && !in_quotes {
            fields.push(current.trim().to_owned());
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().to_owned());
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParseOptions;

    #[test]
    fn test_basic_csv() {
        let csv = "Name,Age,City\nAlice,30,NYC\nBob,25,LA\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        let table = doc.sections.iter().flat_map(|s| &s.content).find_map(|b| {
            if let ContentBlock::Table(t) = b {
                Some(t)
            } else {
                None
            }
        });
        assert!(table.is_some());
        let t = table.unwrap();
        assert_eq!(t.headers, vec!["Name", "Age", "City"]);
        assert_eq!(t.rows.len(), 2);
    }

    #[test]
    fn test_quoted_csv() {
        let csv = "Name,Description\n\"Smith, John\",\"He said \"\"hello\"\"\"\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        let table = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .find_map(|b| {
                if let ContentBlock::Table(t) = b {
                    Some(t)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(table.rows[0][0], "Smith, John");
        assert_eq!(table.rows[0][1], "He said \"hello\"");
    }

    #[test]
    fn test_tab_delimited() {
        let tsv = "A\tB\tC\n1\t2\t3\n";
        let doc = parse(tsv, &ParseOptions::default()).unwrap();
        let table = doc
            .sections
            .iter()
            .flat_map(|s| &s.content)
            .find_map(|b| {
                if let ContentBlock::Table(t) = b {
                    Some(t)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(table.headers.len(), 3);
    }
}
