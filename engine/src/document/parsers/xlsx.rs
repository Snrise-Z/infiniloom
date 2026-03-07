//! XLSX/XLS spreadsheet parser using the calamine crate.
//!
//! Converts spreadsheet files into a Document where each sheet becomes
//! a Section containing a Table. The first row of each sheet is treated
//! as headers when it appears to contain mostly text/non-numeric values.

use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader};

use crate::document::types::*;
use crate::document::ParseOptions;
use crate::error::InfiniloomError;

/// Parse an XLSX/XLS file from raw bytes into a [`Document`].
pub fn parse(content: &[u8], _options: &ParseOptions) -> Result<Document, InfiniloomError> {
    let cursor = Cursor::new(content);
    let mut workbook = open_workbook_auto_from_rs(cursor)
        .map_err(|e| InfiniloomError::invalid_input(format!("Failed to open spreadsheet: {e}")))?;

    let mut doc = Document::new("", DocumentFormat::Xlsx);
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();

    if sheet_names.is_empty() {
        return Ok(doc);
    }

    for sheet_name in &sheet_names {
        let range = workbook.worksheet_range(sheet_name).map_err(|e| {
            InfiniloomError::invalid_input(format!("Failed to read sheet \"{sheet_name}\": {e}"))
        })?;

        let all_rows: Vec<Vec<String>> = range
            .rows()
            .map(|row| row.iter().map(cell_to_string).collect())
            .collect();

        if all_rows.is_empty() {
            continue;
        }

        let (headers, data_rows) = if all_rows.len() == 1 {
            // Single row: treat as headers with no data
            (all_rows[0].clone(), Vec::new())
        } else if looks_like_header(&all_rows[0], range.rows().next().unwrap_or(&[])) {
            (all_rows[0].clone(), all_rows[1..].to_vec())
        } else {
            // No header row detected; generate column names
            let col_count = all_rows.iter().map(|r| r.len()).max().unwrap_or(0);
            let generated: Vec<String> = (0..col_count)
                .map(|i| format!("Column {}", i + 1))
                .collect();
            (generated, all_rows)
        };

        // Pad ragged rows to match header width
        let header_len = headers.len();
        let padded_rows: Vec<Vec<String>> = data_rows
            .into_iter()
            .map(|mut row| {
                row.resize(header_len, String::new());
                row
            })
            .collect();

        let mut section = Section::new(1, sheet_name.clone());
        section.content.push(ContentBlock::Table(Table {
            caption: None,
            headers,
            rows: padded_rows,
            alignments: Vec::new(),
        }));
        doc.sections.push(section);
    }

    Ok(doc)
}

/// Convert a calamine `Data` cell to a string representation.
fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            // Use integer representation when the float has no fractional part
            if f.fract() == 0.0 && f.is_finite() {
                format!("{:.0}", f)
            } else {
                f.to_string()
            }
        },
        Data::String(s) => s.clone(),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(dt) => format!("datetime({dt})"),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("#ERROR({e:?})"),
        Data::Empty => String::new(),
    }
}

/// Heuristic: a row "looks like a header" if more than half of its non-empty
/// cells are text (not purely numeric or boolean).
fn looks_like_header(row_strings: &[String], row_data: &[Data]) -> bool {
    if row_data.is_empty() {
        return false;
    }
    let non_empty: Vec<&Data> = row_data
        .iter()
        .filter(|c| !matches!(c, Data::Empty))
        .collect();
    if non_empty.is_empty() {
        return false;
    }
    let text_count = non_empty
        .iter()
        .filter(|c| matches!(c, Data::String(_)))
        .count();

    // Also check that no cell in the row is empty string after trimming
    let all_non_blank = row_strings.iter().all(|s| !s.trim().is_empty());

    // At least half are text strings, or all cells are non-blank text
    text_count * 2 >= non_empty.len() || (all_non_blank && text_count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_to_string_variants() {
        assert_eq!(cell_to_string(&Data::Int(42)), "42");
        assert_eq!(cell_to_string(&Data::Float(3.14)), "3.14");
        assert_eq!(cell_to_string(&Data::Float(10.0)), "10");
        assert_eq!(cell_to_string(&Data::String("hello".into())), "hello");
        assert_eq!(cell_to_string(&Data::Bool(true)), "true");
        assert_eq!(cell_to_string(&Data::Empty), "");
    }

    #[test]
    fn test_looks_like_header_text_row() {
        let strings = vec!["Name".into(), "Age".into(), "City".into()];
        let data = vec![
            Data::String("Name".into()),
            Data::String("Age".into()),
            Data::String("City".into()),
        ];
        assert!(looks_like_header(&strings, &data));
    }

    #[test]
    fn test_looks_like_header_numeric_row() {
        let strings = vec!["1".into(), "2".into(), "3".into()];
        let data = vec![Data::Int(1), Data::Int(2), Data::Int(3)];
        assert!(!looks_like_header(&strings, &data));
    }

    #[test]
    fn test_looks_like_header_mixed_row() {
        let strings = vec!["Name".into(), "42".into()];
        let data = vec![Data::String("Name".into()), Data::Int(42)];
        // One of two non-empty cells is text => exactly half => true
        assert!(looks_like_header(&strings, &data));
    }

    #[test]
    fn test_looks_like_header_empty() {
        let strings: Vec<String> = vec![];
        let data: Vec<Data> = vec![];
        assert!(!looks_like_header(&strings, &data));
    }
}
