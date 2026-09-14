use crate::error::{Result, TfError};
use std::collections::BTreeSet;
use std::ops::Range;

const BEGIN: &str = "# BEGIN TeeForge-CD managed scoop";
const END: &str = "# END TeeForge-CD managed scoop";

fn invalid(reason: &str) -> TfError {
    TfError::new(format!(
        "OMK 作用域配置无效，未覆盖 [Invalid OMK scope; not overwritten]: {reason}"
    ))
}

// 只用于识别注释和扩展表头；TOML 语法和数值仍由解析器验证。
// Identify comments and extension headers only; the TOML parser validates syntax and values.
fn lexical_classes(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut classes = vec![0; bytes.len()];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            while i < bytes.len() && bytes[i] != b'\n' {
                classes[i] = 2;
                i += 1;
            }
        } else if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let triple = bytes.get(i..i + 3) == Some(&[quote; 3]);
            let start = i;
            i += if triple { 3 } else { 1 };
            while i < bytes.len() {
                if quote == b'"' && bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                } else if bytes[i] == quote {
                    let mut count = 1;
                    while bytes.get(i + count) == Some(&quote) {
                        count += 1;
                    }
                    if !triple || count >= 3 {
                        i += if triple { count } else { 1 };
                        break;
                    }
                    i += count;
                } else {
                    i += 1;
                }
            }
            classes[start..i].fill(1);
        } else {
            i += 1;
        }
    }
    classes
}

fn parse_view(text: &str, classes: &[u8]) -> Result<String> {
    let alias = (0..1000)
        .map(|n| format!("_o{n:03}"))
        .find(|alias| !text.contains(alias))
        .ok_or_else(|| invalid("extension namespace collision"))?;
    let mut view = text.to_owned();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start_matches([' ', '\t']);
        let start = offset + line.len() - trimmed.len();
        if trimmed.starts_with("[scoop.") && classes[start] == 0 {
            // 等长替换使解析 span 与原文件字节位置一致。
            // Equal-length aliases preserve byte offsets in the original file.
            view.replace_range(start + 1..start + 6, &alias);
        }
        offset += line.len();
    }
    Ok(view)
}

fn markers(text: &str, classes: &[u8], array: &Range<usize>) -> Result<Option<Range<usize>>> {
    let mut begin = None;
    let mut end = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        let start = offset + line.len() - line.trim_start().len();
        if classes.get(start) == Some(&2)
            && (trimmed.starts_with(BEGIN) || trimmed.starts_with(END))
            && trimmed != BEGIN
            && trimmed != END
        {
            return Err(invalid("damaged managed marker"));
        }
        if (trimmed == BEGIN || trimmed == END) && classes.get(start) == Some(&2) {
            if offset <= array.start || offset + line.len() > array.end {
                return Err(invalid("managed marker outside scoop"));
            }
            let slot = if trimmed == BEGIN {
                &mut begin
            } else {
                &mut end
            };
            if slot.replace(offset..offset + line.len()).is_some() {
                return Err(invalid("duplicate managed marker"));
            }
        }
        offset += line.len();
    }
    match (begin, end) {
        (None, None) => Ok(None),
        (Some(begin), Some(end)) if begin.end <= end.start => Ok(Some(begin.start..end.end)),
        _ => Err(invalid("incomplete or reversed managed markers")),
    }
}

pub(crate) fn render(text: &str, packages: &[String]) -> Result<String> {
    let classes = lexical_classes(text);
    let view = parse_view(text, &classes)?;
    // 解析错误可能包含整行配置，日志只报告类别。
    // Parser errors may expose configuration lines; report only the category.
    let document = toml_edit::ImDocument::parse(&view).map_err(|_| invalid("TOML syntax"))?;
    if let Some(version) = document.get("version")
        && !matches!(version.as_integer(), Some(0 | 1))
    {
        return Err(invalid("unsupported version"));
    }
    let array = document
        .get("scoop")
        .and_then(|value| value.as_array())
        .ok_or_else(|| invalid("scoop must be a string array"))?;
    let span = array.span().ok_or_else(|| invalid("missing scoop span"))?;
    let managed = markers(text, &classes, &span)?;
    let mut manual = BTreeSet::new();
    let mut last_manual = None;
    for value in array.iter() {
        let name = value
            .as_str()
            .ok_or_else(|| invalid("non-string scoop entry"))?;
        let value_span = value.span().ok_or_else(|| invalid("missing entry span"))?;
        if let Some(region) = &managed {
            if value_span.start < region.end && value_span.end > region.start {
                if value_span.start < region.start || value_span.end > region.end {
                    return Err(invalid("entry crosses managed marker"));
                }
                continue;
            }
        }
        manual.insert(name.trim().to_owned());
        last_manual = Some(value_span);
    }
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let generated: BTreeSet<_> = packages
        .iter()
        .filter(|name| !manual.contains(name.as_str()))
        .collect();
    let mut block = format!("  {BEGIN}{newline}");
    for name in generated {
        // 扫描器已校验包名，仍使用 JSON 字符串转义作为 TOML 基本字符串。
        // Package names are validated by the scanner; JSON escaping also forms a TOML basic string.
        block.push_str(&format!(
            "  {},{newline}",
            serde_json::to_string(name).map_err(|_| invalid("package encoding"))?
        ));
    }
    block.push_str(&format!("  {END}{newline}"));
    let mut result = text.to_owned();
    if let Some(region) = managed {
        result.replace_range(region, &block);
    } else {
        let close = span.end - 1;
        // 在最后一个手动值后补逗号，不移动行尾注释。
        // Insert the separator after the value without moving trailing comments.
        let comma = last_manual
            .as_ref()
            .filter(|value| {
                !(value.end..close).any(|i| classes[i] == 0 && text.as_bytes()[i] == b',')
            })
            .map(|value| value.end);
        let prefix = if text[..close].ends_with('\n') {
            ""
        } else {
            newline
        };
        result.insert_str(close, &format!("{prefix}{block}"));
        if let Some(at) = comma {
            result.insert(at, ',');
        }
    }
    // 复核生成结果；不把扩展表头映射写回文件。
    // Validate the result without persisting the extension-header aliases.
    let validation = parse_view(&result, &lexical_classes(&result))?;
    toml_edit::ImDocument::parse(validation).map_err(|_| invalid("rendered TOML syntax"))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_manual_comments_extensions_and_is_idempotent() {
        let input = "# title\nversion = 1\nscoop = ['manual.app' # keep\n]\n[main]\nenabled = true\n[scoop.manual.app]\nmode = 'strict'\n";
        let output = render(
            input,
            &["new.app".into(), "manual.app".into(), "new.app".into()],
        )
        .unwrap();
        assert!(output.contains("'manual.app', # keep"));
        assert!(output.ends_with("[main]\nenabled = true\n[scoop.manual.app]\nmode = 'strict'\n"));
        assert_eq!(output, render(&output, &["new.app".into()]).unwrap());
        let removed = render(&output, &[]).unwrap();
        assert!(!removed.contains("\"new.app\""));
        assert!(removed.contains("'manual.app'"));
    }

    #[test]
    fn supports_old_versions_inline_arrays_and_crlf() {
        for version in ["", "version = 0\r\n", "version = 1\r\n"] {
            let input = format!("{version}scoop = [\"manual.app\"] # keep\r\n");
            let output = render(&input, &["new.app".into()]).unwrap();
            assert!(output.starts_with(version));
            assert!(output.ends_with("] # keep\r\n"));
            assert_eq!(output, render(&output, &["new.app".into()]).unwrap());
        }
    }

    #[test]
    fn rejects_invalid_inputs_without_exposing_contents() {
        for input in [
            "scoop = [1]",
            "scoop = {}",
            "version = 2\nscoop = []",
            "scoop = []\nscoop = []",
            "other = []",
            "scoop = [\n# BEGIN TeeForge-CD managed scoop\n]",
            "# BEGIN TeeForge-CD managed scoop\nscoop = []",
            "scoop = [\n# END TeeForge-CD managed scoop\n# BEGIN TeeForge-CD managed scoop\n]",
        ] {
            assert!(render(input, &[]).is_err(), "{input}");
        }
    }

    #[test]
    fn marker_text_in_multiline_strings_is_not_a_marker() {
        let input =
            "note = '''\n[scoop.not.a.table]\n# BEGIN TeeForge-CD managed scoop\n'''\nscoop = []\n";
        let output = render(input, &[]).unwrap();
        assert!(output.starts_with(input.split("scoop = []").next().unwrap()));
        assert_eq!(output, render(&output, &[]).unwrap());
    }

    #[test]
    fn preserves_manual_entries_on_both_sides_of_managed_region() {
        let input = format!(
            "scoop = [\n'before.app', # before\n  {BEGIN}\n  'old.app',\n  {END}\n'after.app' # after\n]\n"
        );
        let output = render(&input, &["new.app".into(), "after.app".into()]).unwrap();
        assert!(output.contains("'before.app', # before"));
        assert!(output.contains("'after.app' # after"));
        assert!(!output.contains("old.app"));
        assert_eq!(output, render(&output, &["new.app".into()]).unwrap());
        assert!(render(&input.replace(BEGIN, &format!("{BEGIN} broken")), &[]).is_err());
    }
}
