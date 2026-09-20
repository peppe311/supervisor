use super::*;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    pub start: usize,
    pub end: usize,
}

fn utf16_slice<'a>(text: &'a str, selection: &Selection) -> Result<&'a str, String> {
    if selection.start > selection.end {
        return Err("Invalid editor selection.".into());
    }
    let mut position = 0;
    let mut start = None;
    let mut end = None;
    for (byte, character) in text
        .char_indices()
        .chain(std::iter::once((text.len(), '\0')))
    {
        if position == selection.start {
            start = Some(byte);
        }
        if position == selection.end {
            end = Some(byte);
        }
        position += character.len_utf16();
    }
    match (start, end) {
        (Some(start), Some(end)) => Ok(&text[start..end]),
        _ => {
            Err("Editor selection is stale or splits a Unicode character. Select it again.".into())
        }
    }
}

impl FileEditor {
    pub(crate) fn select(
        &mut self,
        id: &str,
        version: u64,
        selection: Selection,
    ) -> Result<(), String> {
        let doc = self.document(id)?;
        if doc.version != version {
            return Err("The selection belongs to a different editor revision.".into());
        }
        utf16_slice(&doc.file.content, &selection)?;
        self.selections.insert(id.into(), (version, selection));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_selection_handles_surrogates_and_rejects_invalid_boundaries() {
        assert_eq!(
            utf16_slice("a🦀b\n", &Selection { start: 1, end: 3 }).unwrap(),
            "🦀"
        );
        for selection in [
            Selection { start: 2, end: 3 },
            Selection { start: 0, end: 99 },
            Selection { start: 4, end: 1 },
        ] {
            assert!(utf16_slice("a🦀b", &selection).is_err());
        }
    }
}
