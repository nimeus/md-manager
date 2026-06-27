//! Header-aware markdown chunking for full-text search.
//!
//! Splits a document at markdown heading boundaries (carrying a heading breadcrumb),
//! never splits inside a fenced code block, and further splits very large sections at
//! blank lines so each chunk stays roughly within `max_chars`.

/// One indexed chunk of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub index: i32,
    /// Breadcrumb of enclosing headings, e.g. `"Guide > Setup"`.
    pub heading_path: String,
    pub content: String,
}

const DEFAULT_MAX_CHARS: usize = 1500;

/// Chunk with the default target size.
pub fn chunk_markdown(content: &str) -> Vec<Chunk> {
    chunk_markdown_with(content, DEFAULT_MAX_CHARS)
}

/// Chunk `content`, targeting at most ~`max_chars` characters per chunk.
pub fn chunk_markdown_with(content: &str, max_chars: usize) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut path: Vec<(u8, String)> = Vec::new();
    let mut buf = String::new();
    let mut in_fence = false;
    let mut next_index: i32 = 0;

    let flush = |buf: &mut String, path: &[(u8, String)], next_index: &mut i32, chunks: &mut Vec<Chunk>| {
        if buf.trim().is_empty() {
            buf.clear();
            return;
        }
        chunks.push(Chunk {
            index: *next_index,
            heading_path: breadcrumb(path),
            content: buf.trim_end().to_string(),
        });
        *next_index += 1;
        buf.clear();
    };

    for line in content.lines() {
        let trimmed = line.trim_start();
        let is_fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if is_fence {
            in_fence = !in_fence;
            buf.push_str(line);
            buf.push('\n');
            continue;
        }

        if !in_fence {
            if let Some((level, _text)) = parse_heading(line) {
                // Close the previous section before starting this heading's chunk.
                flush(&mut buf, &path, &mut next_index, &mut chunks);
                // Pop same-or-deeper headings, then push this one.
                while path.last().map(|(l, _)| *l >= level).unwrap_or(false) {
                    path.pop();
                }
                if let Some((_, text)) = parse_heading(line) {
                    path.push((level, text));
                }
                buf.push_str(line);
                buf.push('\n');
                continue;
            }
        }

        buf.push_str(line);
        buf.push('\n');

        // Size-based split, only at a blank line outside a fence.
        if !in_fence && line.trim().is_empty() && buf.len() >= max_chars {
            flush(&mut buf, &path, &mut next_index, &mut chunks);
        }
    }

    flush(&mut buf, &path, &mut next_index, &mut chunks);
    chunks
}

fn breadcrumb(path: &[(u8, String)]) -> String {
    path.iter()
        .map(|(_, t)| t.as_str())
        .collect::<Vec<_>>()
        .join(" > ")
}

/// If `line` is an ATX heading (`#`..`######` followed by a space), return its level + text.
fn parse_heading(line: &str) -> Option<(u8, String)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.starts_with(' ') {
        return None;
    }
    Some((hashes as u8, rest.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_headings_with_breadcrumb() {
        let md = "# Title\nintro\n\n## Setup\ndo this\n\n## Usage\ndo that\n";
        let chunks = chunk_markdown(md);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].heading_path, "Title");
        assert_eq!(chunks[1].heading_path, "Title > Setup");
        assert_eq!(chunks[2].heading_path, "Title > Usage");
        assert!(chunks[1].content.contains("do this"));
    }

    #[test]
    fn does_not_treat_hash_in_code_fence_as_heading() {
        let md = "# Real\ntext\n\n```\n# not a heading\nmore\n```\ntail\n";
        let chunks = chunk_markdown(md);
        // Only one real heading => one chunk; the fenced "# not a heading" stays inside it.
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading_path, "Real");
        assert!(chunks[0].content.contains("# not a heading"));
    }

    #[test]
    fn empty_input_yields_no_chunks() {
        assert!(chunk_markdown("").is_empty());
        assert!(chunk_markdown("   \n  \n").is_empty());
    }
}
