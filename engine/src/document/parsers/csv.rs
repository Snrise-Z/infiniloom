//! CSV document parser.
//!
//! Converts CSV content into a Document with a single table section.
//! Supports auto-detection of delimiters (comma, tab, pipe, semicolon),
//! comment lines starting with `#`, and ragged rows (padded with empty strings).

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Maximum number of rows to parse from a CSV file.
/// Prevents unbounded memory allocation from extremely large files.
const MAX_CSV_ROWS: usize = 1_000_000;

/// Maximum number of columns (cells per row) to parse.
/// Prevents OOM from a single row with millions of delimiter-separated fields.
const MAX_CSV_COLUMNS: usize = 10_000;

/// Parse CSV content into a Document containing a table.
pub fn parse(content: &str, _options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let mut doc = Document::new("", DocumentFormat::Csv);
    let mut lines = content.lines();

    // First non-empty, non-comment line is headers
    let header_line = loop {
        match lines.next() {
            Some(line) if is_skippable(line) => continue,
            Some(line) => break line,
            None => return Ok(doc),
        }
    };

    let delimiter = detect_delimiter(header_line);
    let headers: Vec<String> = split_csv_line(header_line, delimiter);
    let header_len = headers.len();
    let mut rows: Vec<Vec<String>> = Vec::new();

    for line in lines {
        if is_skippable(line) {
            continue;
        }
        if rows.len() >= MAX_CSV_ROWS {
            break;
        }
        let mut row = split_csv_line(line, delimiter);
        // Pad ragged rows to match header width
        row.resize(header_len, String::new());
        rows.push(row);
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

/// Returns true for lines that should be skipped: empty or comment lines.
fn is_skippable(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#')
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
    // Prefer comma as default when tied, then tab, then semicolon, then pipe
    if comma_count == max {
        ','
    } else if tab_count == max {
        '\t'
    } else if semicolon_count == max {
        ';'
    } else {
        '|'
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
            fields.push(sanitize_cell(&current));
            if fields.len() >= MAX_CSV_COLUMNS {
                break;
            }
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    if fields.len() < MAX_CSV_COLUMNS {
        fields.push(sanitize_cell(&current));
    }
    fields
}

/// Sanitize a cell value to prevent formula injection in spreadsheet applications.
///
/// When a cell value starts with `=`, `+`, `-`, `@`, `|`, or a tab character
/// (before or after whitespace trimming), it is prefixed with a single quote
/// to neutralize potential formula execution when the output is opened in a
/// spreadsheet application.
pub(super) fn sanitize_cell(value: &str) -> String {
    let trimmed = value.trim();
    // Check the raw value first (catches leading tabs that trim() would remove),
    // then also check the trimmed value.
    if let Some(first) = value.chars().next() {
        if matches!(first, '=' | '+' | '-' | '@' | '|' | '\t') {
            return format!("'{trimmed}");
        }
    }
    if let Some(first) = trimmed.chars().next() {
        if matches!(first, '=' | '+' | '-' | '@' | '|' | '\t') {
            return format!("'{trimmed}");
        }
    }
    trimmed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::ParseOptions;

    /// Helper to extract the first table from a parsed document.
    fn first_table(doc: &Document) -> &Table {
        doc.sections
            .iter()
            .flat_map(|s| &s.content)
            .find_map(|b| {
                if let ContentBlock::Table(t) = b {
                    Some(t)
                } else {
                    None
                }
            })
            .expect("expected at least one table")
    }

    #[test]
    fn test_basic_csv() {
        let csv = "Name,Age,City\nAlice,30,NYC\nBob,25,LA\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.headers, vec!["Name", "Age", "City"]);
        assert_eq!(t.rows.len(), 2);
    }

    #[test]
    fn test_quoted_csv() {
        let csv = "Name,Description\n\"Smith, John\",\"He said \"\"hello\"\"\"\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.rows[0][0], "Smith, John");
        assert_eq!(t.rows[0][1], "He said \"hello\"");
    }

    #[test]
    fn test_tab_delimited() {
        let tsv = "A\tB\tC\n1\t2\t3\n";
        let doc = parse(tsv, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.headers, vec!["A", "B", "C"]);
        assert_eq!(t.rows[0], vec!["1", "2", "3"]);
    }

    #[test]
    fn test_pipe_delimited() {
        let data = "Name|Score|Grade\nAlice|95|A\nBob|82|B\n";
        let doc = parse(data, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.headers, vec!["Name", "Score", "Grade"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0][0], "Alice");
    }

    #[test]
    fn test_semicolon_delimited() {
        let data = "Name;Age;City\nAlice;30;NYC\n";
        let doc = parse(data, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.headers, vec!["Name", "Age", "City"]);
        assert_eq!(t.rows[0], vec!["Alice", "30", "NYC"]);
    }

    #[test]
    fn test_comment_lines_skipped() {
        let csv = "# This is a comment\nName,Age\n# Another comment\nAlice,30\nBob,25\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.headers, vec!["Name", "Age"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[0], vec!["Alice", "30"]);
    }

    #[test]
    fn test_ragged_rows_padded() {
        let csv = "A,B,C\n1,2\n4,5,6\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.headers, vec!["A", "B", "C"]);
        // Short row should be padded to 3 columns
        assert_eq!(t.rows[0], vec!["1", "2", ""]);
        assert_eq!(t.rows[1], vec!["4", "5", "6"]);
    }

    #[test]
    fn test_empty_content() {
        let doc = parse("", &ParseOptions::default()).unwrap();
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn test_only_comments() {
        let csv = "# comment 1\n# comment 2\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        assert!(doc.sections.is_empty());
    }

    #[test]
    fn test_formula_injection_sanitized() {
        let csv = "Name,Value\n=CMD('calc'),normal\n+1-1,ok\n-1+1,ok\n@SUM(A1),ok\n|cmd,ok\n\tindented,ok\nplain,safe\n";
        let doc = parse(csv, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert_eq!(t.rows[0][0], "'=CMD('calc')", "= prefix should be sanitized");
        assert_eq!(t.rows[0][1], "normal", "normal cell should be unchanged");
        assert_eq!(t.rows[1][0], "'+1-1", "+ prefix should be sanitized");
        assert_eq!(t.rows[2][0], "'-1+1", "- prefix should be sanitized");
        assert_eq!(t.rows[3][0], "'@SUM(A1)", "@ prefix should be sanitized");
        assert_eq!(t.rows[4][0], "'|cmd", "| prefix should be sanitized");
        assert_eq!(t.rows[5][0], "'indented", "tab prefix should be sanitized");
        assert_eq!(t.rows[6][0], "plain", "plain text should be unchanged");
        assert_eq!(t.rows[6][1], "safe", "safe cell should be unchanged");
    }

    // Regression test for #127: CSV parser must limit columns to prevent DoS
    #[test]
    fn test_column_limit_enforced() {
        // Build a CSV line with more than MAX_CSV_COLUMNS fields
        let many_headers: String = (0..MAX_CSV_COLUMNS + 100)
            .map(|i| format!("H{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let doc = parse(&many_headers, &ParseOptions::default()).unwrap();
        let t = first_table(&doc);
        assert!(
            t.headers.len() <= MAX_CSV_COLUMNS,
            "columns should be capped at MAX_CSV_COLUMNS, got {}",
            t.headers.len()
        );
    }
}
