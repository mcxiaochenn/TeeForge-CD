//! 极简 ELF64 动态符号读取 [Minimal ELF64 dynsym reader]
//!
//! 只用于诊断：列出厂商库导出的方法，判断是否存在必须先调用的初始化入口、
//! `send_cmd` 的其它重载，或构造函数的不同变体。
//! 不解析重定位、不加载库、不执行任何厂商代码。
//!
//! Values are read through bounds-checked accessors so a malformed file returns an
//! error instead of panicking.

use crate::error::{Result, TfError};
use std::fs;

const SHT_DYNSYM: u32 = 11;
const SYMBOL_SIZE: usize = 24;
const STT_FUNC: u8 = 2;

/// 读取 `path` 中名字包含 `needle` 的函数符号，去重排序。
pub(crate) fn function_symbols(path: &str, needle: &str) -> Result<Vec<String>> {
    let data = fs::read(path)
        .map_err(|error| TfError::new(format!("读取 {path} 失败 [read failed]: {error}")))?;
    parse(&data, needle)
}

fn parse(data: &[u8], needle: &str) -> Result<Vec<String>> {
    if data.get(..4) != Some(b"\x7fELF") {
        return Err(fail("不是 ELF 文件 [not an ELF file]"));
    }
    if data.get(4) != Some(&2) {
        return Err(fail("只支持 64 位 ELF [only ELF64 is supported]"));
    }
    if data.get(5) != Some(&1) {
        return Err(fail("只支持小端 ELF [only little-endian is supported]"));
    }

    let section_offset = usize_of(u64_at(data, 0x28)?)?;
    // e_shentsize(0x3a) 与 e_shnum(0x3c) 都是 u16，不能按 u64 读。
    let section_size = usize_of(u64::from(u16_at(data, 0x3a).unwrap_or(64)))?;
    let section_count = usize_of(u64::from(u16_at(data, 0x3c).unwrap_or(0)))?;
    if section_size < 64 || section_count == 0 {
        return Err(fail("节表不可用 [section table unavailable]"));
    }

    // (类型, 偏移, 长度, 关联字符串表索引)
    let mut sections = Vec::with_capacity(section_count);
    for index in 0..section_count {
        let base = section_offset
            .checked_add(index.checked_mul(section_size).ok_or_else(overflow)?)
            .ok_or_else(overflow)?;
        sections.push((
            u32_at(data, base + 4)?,
            usize_of(u64_at(data, base + 24)?)?,
            usize_of(u64_at(data, base + 32)?)?,
            u32_at(data, base + 40)? as usize,
        ));
    }

    let mut names = Vec::new();
    for index in 0..sections.len() {
        let (kind, offset, size, link) = sections[index];
        if kind != SHT_DYNSYM {
            continue;
        }
        let Some((_, string_offset, string_size, _)) = sections.get(link) else {
            continue;
        };
        let Some(strings) = data.get(*string_offset..string_offset.saturating_add(*string_size))
        else {
            continue;
        };
        for symbol in 0..(size / SYMBOL_SIZE) {
            let Some(entry) = offset.checked_add(symbol * SYMBOL_SIZE) else {
                break;
            };
            let Some(name_offset) = u32_at(data, entry)
                .ok()
                .and_then(|value| usize::try_from(value).ok())
            else {
                continue;
            };
            let Some(info) = data.get(entry + 4) else {
                continue;
            };
            if info & 0x0f != STT_FUNC {
                continue;
            }
            if let Some(name) = read_name(strings, name_offset) {
                if name.contains(needle) {
                    names.push(name);
                }
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn read_name(strings: &[u8], offset: usize) -> Option<String> {
    let tail = strings.get(offset..)?;
    let end = tail.iter().position(|byte| *byte == 0)?;
    std::str::from_utf8(&tail[..end]).ok().map(str::to_owned)
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        data.get(offset..offset + 2)
            .ok_or_else(|| fail("ELF 读取越界 [out of bounds]"))?
            .try_into()
            .map_err(|_| fail("ELF 字段长度错误 [bad field width]"))?,
    ))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or_else(|| fail("ELF 读取越界 [out of bounds]"))?
            .try_into()
            .map_err(|_| fail("ELF 字段长度错误 [bad field width]"))?,
    ))
}

fn u64_at(data: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(
        data.get(offset..offset + 8)
            .ok_or_else(|| fail("ELF 读取越界 [out of bounds]"))?
            .try_into()
            .map_err(|_| fail("ELF 字段长度错误 [bad field width]"))?,
    ))
}

/// `u64` 到 `usize`；32 位目标上超出范围视为越界。
fn usize_of(value: u64) -> Result<usize> {
    usize::try_from(value).map_err(|_| fail("偏移超出地址空间 [offset exceeds address space]"))
}

fn overflow() -> TfError {
    fail("节表偏移溢出 [section offset overflow]")
}

fn fail(message: impl Into<String>) -> TfError {
    TfError::new(format!("符号表 [symbol table]: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_elf_data_is_rejected() {
        assert!(parse(b"not an elf file", "send").is_err());
    }

    #[test]
    fn truncated_header_is_rejected() {
        assert!(parse(b"\x7fELF\x02\x01", "send").is_err());
    }

    #[test]
    fn zero_section_table_is_rejected() {
        let mut data = vec![0u8; 64];
        data[..4].copy_from_slice(b"\x7fELF");
        data[4] = 2;
        data[5] = 1;
        assert!(parse(&data, "send").is_err());
    }

    #[test]
    fn name_reader_stops_at_nul() {
        assert_eq!(read_name(b"send_cmd\0x", 0).as_deref(), Some("send_cmd"));
        assert!(read_name(b"send", 9).is_none());
    }
}
