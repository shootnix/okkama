//! Iterative glob matching and traversal. Symlinks are never followed as directories.
use crate::host::{
    Error,
    ProviderError,
    native_fs::{fail, host},
    path_syntax::{Spec, joined},
    *,
};
use okkama_runtime_abi::Native;
use std::collections::{BTreeSet, HashSet};
enum Token {
    Any,
    One,
    Literal(char),
    Class(bool, Vec<(char, char)>),
}

fn glob_segment(native: Native, segment: &str) -> Result<Option<Vec<Token>>, ProviderError> {
    if segment != "**" && segment.contains("**") {
        return Err(fail(
            native,
            "invalid_input",
            "Recursive ** must be a complete path segment",
        ));
    }
    if segment == "**" {
        return Ok(None);
    }
    let characters: Vec<char> = segment.chars().collect();
    let mut tokens = Vec::new();
    let mut position = 0;
    while position < characters.len() {
        match characters[position] {
            '*' => tokens.push(Token::Any),
            '?' => tokens.push(Token::One),
            '[' => {
                let mut ending = position + 1;
                while ending < characters.len() && characters[ending] != ']' {
                    ending += 1;
                }
                if ending >= characters.len() {
                    return Err(fail(
                        native,
                        "invalid_input",
                        "Unclosed glob character class",
                    ));
                }
                let mut from = position + 1;
                if from == ending {
                    return Err(fail(native, "invalid_input", "Empty glob character class"));
                }
                let negated = characters[from] == '!';
                if negated {
                    from += 1;
                }
                if from == ending {
                    return Err(fail(
                        native,
                        "invalid_input",
                        "Empty negated glob character class",
                    ));
                }
                let mut ranges = Vec::new();
                let mut index = from;
                while index < ending {
                    let first = characters[index];
                    index += 1;
                    if index + 1 < ending && characters[index] == '-' {
                        let last = characters[index + 1];
                        if first > last {
                            return Err(fail(
                                native,
                                "invalid_input",
                                "Descending glob character range",
                            ));
                        }
                        ranges.push((first, last));
                        index += 2;
                    } else {
                        ranges.push((first, first));
                    }
                }
                tokens.push(Token::Class(negated, ranges));
                position = ending;
            }
            c => tokens.push(Token::Literal(c)),
        }
        position += 1;
    }
    Ok(Some(tokens))
}

fn matches(tokens: &[Token], name: &str) -> bool {
    let chars: Vec<char> = name.chars().collect();
    let mut previous = vec![false; chars.len() + 1];
    previous[0] = true;
    let mut next = vec![false; chars.len() + 1];
    for token in tokens {
        next.fill(false);
        if matches!(token, Token::Any) {
            next[0] = previous[0];
            for i in 1..=chars.len() {
                next[i] = previous[i] || next[i - 1];
            }
        } else {
            for (i, c) in chars.iter().copied().enumerate() {
                let accepts = match token {
                    Token::One => true,
                    Token::Literal(v) => *v == c,
                    Token::Class(negated, ranges) => {
                        *negated != ranges.iter().any(|(lo, hi)| (*lo..=*hi).contains(&c))
                    }
                    Token::Any => unreachable!(),
                };
                next[i + 1] = previous[i] && accepts;
            }
        }
        std::mem::swap(&mut previous, &mut next);
    }
    previous[chars.len()]
}
impl Heap {
    pub(crate) fn glob_execute(&mut self, n: Native, a: &[Slot]) -> Result<Slot, ProviderError> {
        let root = self.fs_path(n, a[0])?;
        let root_text = String::from_utf8_lossy(&self.native_bytes(a[0])?).into_owned();
        let pattern = self.text(a[1])?;
        if pattern.is_empty() || pattern.split('/').any(str::is_empty) {
            return Err(fail(
                n,
                "invalid_input",
                "Glob must contain nonempty relative segments",
            ));
        }
        let segments = pattern
            .split('/')
            .map(|s| glob_segment(n, s))
            .collect::<Result<Vec<_>, _>>()?;
        let metadata = std::fs::symlink_metadata(&root).map_err(|e| host(n, e))?;
        if !metadata.is_dir() {
            return Err(fail(n, "invalid_input", "Glob root must be a directory"));
        }
        let mut pending = vec![(root, root_text, 0, true, true)];
        let mut visited = HashSet::new();
        let mut result = BTreeSet::new();
        while let Some((path, text, index, directory, is_root)) = pending.pop() {
            if !visited.insert((index, text.clone())) {
                continue;
            }
            if index == segments.len() {
                if !is_root {
                    result.insert(text);
                }
                continue;
            }
            let recursive = segments[index].is_none();
            if recursive {
                pending.push((path.clone(), text.clone(), index + 1, directory, is_root));
            }
            if !directory {
                continue;
            }
            for entry in std::fs::read_dir(&path).map_err(|e| host(n, e))? {
                let entry = entry.map_err(|e| host(n, e))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if segments[index]
                    .as_ref()
                    .is_some_and(|tokens| !matches(tokens, &name))
                {
                    continue;
                }
                let child = entry.path();
                let directory = std::fs::symlink_metadata(&child).is_ok_and(|m| m.is_dir());
                let child_text = joined(Spec::Unix, &[text.clone(), name]);
                pending.push((
                    child,
                    child_text,
                    if recursive { index } else { index + 1 },
                    directory,
                    false,
                ));
            }
        }
        let slots = result
            .into_iter()
            .map(|text| self.allocate_text(text))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(self.allocate_array(slots)?)
    }
}
