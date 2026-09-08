//! Worker-local reuse of normalized inline results. No source text is persisted
//! or included in Debug output; entry count and retained payload are bounded.

use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};

use token::completion::inline::{InlineRequest, MAX_ALTERNATIVES, PREFIX_BUDGET_CHARS};
use token::completion::provider::InlineJob;
use token::config::ProviderConfig;
use token::util::ByteSize;

const ENTRY_LIMIT: usize = 256;
const PAYLOAD_LIMIT: ByteSize = ByteSize::mebibytes(8);

#[derive(Default)]
pub(super) struct InlineCache {
    entries: VecDeque<Entry>,
    payload_bytes: usize,
}

struct Entry {
    request: InlineRequest,
    provider: ProviderConfig,
    prefix_hash: u64,
    texts: Vec<String>,
    served: Vec<String>,
    payload_bytes: usize,
}

fn prefix_hash(prefix: &str) -> u64 {
    let mut hash = DefaultHasher::new();
    prefix.hash(&mut hash);
    hash.finish()
}

impl InlineCache {
    /// Explicit requests refresh the provider. Automatic requests may reuse
    /// exact context or a still-matching remainder after typing/acceptance.
    pub(super) fn get(&mut self, job: &InlineJob) -> Option<Vec<String>> {
        if job.request.explicit {
            return None;
        }
        let hash = prefix_hash(&job.request.prefix);
        let found = self
            .entries
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, entry)| entry.replay(job, hash).map(|texts| (i, texts)));
        let (index, texts) = found?;
        if let Some(entry) = self.entries.remove(index) {
            self.entries.push_back(entry);
        }
        Some(texts)
    }

    pub(super) fn insert(&mut self, job: &InlineJob, texts: &[String], served: &[String]) {
        // A fresh empty/rejected/oversize result must not leave an older answer
        // at this exact context available to the next automatic request.
        if let Some(index) = self.entries.iter().position(|old| old.same_context(job)) {
            if let Some(old) = self.entries.remove(index) {
                self.payload_bytes -= old.payload_bytes;
            }
        }
        if texts.is_empty() || served.is_empty() {
            return;
        }
        let mut entry = Entry {
            request: job.request.clone(),
            provider: job.provider.clone(),
            prefix_hash: prefix_hash(&job.request.prefix),
            texts: texts.iter().take(MAX_ALTERNATIVES).cloned().collect(),
            served: served.iter().take(MAX_ALTERNATIVES).cloned().collect(),
            payload_bytes: 0,
        };
        // Count retained capacities, not just string lengths. Fixed entry
        // metadata is separately bounded by ENTRY_LIMIT.
        let request = &entry.request;
        let provider = &entry.provider;
        entry.payload_bytes = request.prefix.capacity()
            + request.suffix.capacity()
            + request.extra_context.capacity()
                * std::mem::size_of::<token::completion::recency::ContextChunk>()
            + request
                .extra_context
                .iter()
                .map(|chunk| chunk.filename.capacity() + chunk.text.capacity())
                .sum::<usize>()
            + request.language.as_ref().map_or(0, String::capacity)
            + request.file_path.as_ref().map_or(0, |path| path.capacity())
            + provider.url.capacity()
            + provider.model.as_ref().map_or(0, String::capacity)
            + provider.api_key_env.as_ref().map_or(0, String::capacity)
            + entry.texts.capacity() * std::mem::size_of::<String>()
            + entry.texts.iter().map(String::capacity).sum::<usize>()
            + entry.served.capacity() * std::mem::size_of::<String>()
            + entry.served.iter().map(String::capacity).sum::<usize>();
        if entry.payload_bytes > PAYLOAD_LIMIT.as_usize() {
            return;
        }
        while self.entries.len() >= ENTRY_LIMIT
            || self.payload_bytes + entry.payload_bytes > PAYLOAD_LIMIT.as_usize()
        {
            let Some(old) = self.entries.pop_front() else {
                break;
            };
            self.payload_bytes -= old.payload_bytes;
        }
        self.payload_bytes += entry.payload_bytes;
        self.entries.push_back(entry);
    }
}

impl Entry {
    fn same_origin(&self, job: &InlineJob) -> bool {
        let old = &self.request;
        let new = &job.request;
        old.snapshot.document_id == new.snapshot.document_id
            && self.provider == job.provider
            && old.language == new.language
            && old.file_path == new.file_path
            && old.suffix == new.suffix
            && old.extra_context == new.extra_context
    }

    fn same_context(&self, job: &InlineJob) -> bool {
        let old = &self.request;
        let new = &job.request;
        self.same_origin(job)
            && (old.snapshot.line, old.snapshot.column) == (new.snapshot.line, new.snapshot.column)
            && old.prefix == new.prefix
    }

    fn replay(&self, job: &InlineJob, hash: u64) -> Option<Vec<String>> {
        if !self.same_origin(job) {
            return None;
        }
        if self.prefix_hash == hash && self.same_context(job) {
            return Some(self.texts.clone());
        }
        let old = &self.request;
        let new = &job.request;
        let texts: Vec<_> = self
            .served
            .iter()
            .filter_map(|text| {
                let consumed = consumed_at_cursor(text, old, new)?;
                let (typed, remainder) = text.split_at(consumed);
                if remainder.is_empty() {
                    return None;
                }
                // Compare the entire bounded prompt, not merely its hash or
                // latest token. Prefix windows slide after long-line typing.
                old.prefix
                    .chars()
                    .chain(typed.chars())
                    .rev()
                    .take(PREFIX_BUDGET_CHARS)
                    .eq(new.prefix.chars().rev())
                    .then(|| remainder.to_owned())
            })
            .collect();
        (!texts.is_empty()).then_some(texts)
    }
}

/// Locate the new cursor in a candidate using the editor's character-column
/// convention. CRLF advances the row on LF; UTF-8 offsets stay at boundaries.
fn consumed_at_cursor(text: &str, old: &InlineRequest, new: &InlineRequest) -> Option<usize> {
    let target = (new.snapshot.line, new.snapshot.column);
    let mut position = (old.snapshot.line, old.snapshot.column);
    if target <= position {
        return None;
    }
    for (byte, ch) in text.char_indices() {
        if ch == '\n' {
            position = (position.0 + 1, 0);
        } else {
            position.1 += 1;
        }
        if position == target {
            return Some(byte + ch.len_utf8());
        }
        if position > target {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::completion::inline::RequestSnapshot;
    use token::config::ProviderConfig;
    use token::model::DocumentId;

    fn insert(cache: &mut InlineCache, job: &InlineJob, texts: &[String]) {
        cache.insert(job, texts, texts);
    }

    fn job(prefix: &str) -> InlineJob {
        InlineJob {
            request: InlineRequest {
                snapshot: RequestSnapshot {
                    document_id: DocumentId(1),
                    revision: 1,
                    line: 0,
                    column: prefix.chars().count(),
                    request_id: 1,
                },
                prefix: prefix.into(),
                suffix: "\n".into(),
                language: Some("rust".into()),
                file_path: Some("/fixture.rs".into()),
                extra_context: Vec::new(),
                explicit: false,
            },
            provider: ProviderConfig::default(),
            context: None,
        }
    }

    #[test]
    fn replay_consumes_served_indentation_but_exact_hits_keep_raw_candidates() {
        let original = job("    ");
        let mut cache = InlineCache::default();
        cache.insert(
            &original,
            &["first();\n\tsecond();".into()],
            &["first();\n    second();".into()],
        );
        assert_eq!(cache.get(&original).unwrap(), ["first();\n\tsecond();"]);
        let mut next = original.clone();
        next.request.prefix.push_str("first();\n    ");
        next.request.snapshot.line = 1;
        next.request.snapshot.column = 4;
        assert_eq!(cache.get(&next).unwrap(), ["second();"]);
        let rejected = job("different");
        cache.insert(&rejected, &["old();".into()], &["old();".into()]);
        cache.insert(&rejected, &["]".into()], &[]);
        assert!(
            cache.get(&rejected).is_none(),
            "fully rejected results are not cached"
        );
    }

    #[test]
    fn exact_replay_ignores_revision_but_checks_all_generation_context() {
        let original = job("let x = ");
        let mut cache = InlineCache::default();
        insert(&mut cache, &original, &["answer();".into()]);
        let mut request = original.clone();
        request.request.snapshot.revision = 10;
        request.request.snapshot.request_id = 20;
        assert_eq!(cache.get(&request), Some(vec!["answer();".into()]));
        let mut changed = Vec::new();
        let mut other = request.clone();
        other
            .request
            .extra_context
            .push(token::completion::recency::ContextChunk {
                filename: "helper.rs".into(),
                text: "fn helper() {}".into(),
            });
        changed.push(other);
        let mut other = request.clone();
        other.request.prefix = "let y = ".into();
        changed.push(other);
        let mut other = request.clone();
        other.request.suffix = "different".into();
        changed.push(other);
        let mut other = request.clone();
        other.request.language = Some("python".into());
        changed.push(other);
        let mut other = request.clone();
        other.request.file_path = Some("/other.rs".into());
        changed.push(other);
        let mut other = request.clone();
        other.request.snapshot.document_id = DocumentId(2);
        changed.push(other);
        let mut other = request.clone();
        other.request.snapshot.column += 1;
        changed.push(other);
        let mut other = request.clone();
        other.provider.model = Some("other".into());
        changed.push(other);
        let mut other = request.clone();
        other.provider.api_key_env = Some("OTHER_TOKEN".into());
        changed.push(other);
        let mut other = request.clone();
        other.provider.n = 2;
        changed.push(other);
        let mut other = request.clone();
        other.provider.prompt_format = token::completion::prompt::PromptFormat::Qwen;
        changed.push(other);
        let mut other = request;
        other.request.explicit = true;
        changed.push(other);
        for other in changed {
            assert!(cache.get(&other).is_none());
        }
        // A hash collision must still fail the exact prefix comparison.
        cache.entries[0].prefix_hash = prefix_hash("let y = ");
        assert!(cache.get(&job("let y = ")).is_none());
    }

    #[test]
    fn replay_matches_unicode_crlf_and_alternative_prefixes() {
        let original = job("let x = ");
        let mut cache = InlineCache::default();
        insert(
            &mut cache,
            &original,
            &["éclair();\r\nnext();".into(), "elsewhere();".into()],
        );
        assert_eq!(
            cache.get(&job("let x = é")),
            Some(vec!["clair();\r\nnext();".into()])
        );
        let mut newline = job("let x = éclair();\r\n");
        newline.request.snapshot.line = 1;
        newline.request.snapshot.column = 0;
        assert_eq!(cache.get(&newline), Some(vec!["next();".into()]));
        let mut changed = newline.clone();
        changed.request.prefix = "let y = éclair();\r\n".into();
        assert!(cache.get(&changed).is_none());
        assert!(cache.get(&job("let x = different")).is_none());
        assert_eq!(
            cache.get(&original).unwrap().len(),
            2,
            "backspace can restore alternatives"
        );
    }

    #[test]
    fn replay_handles_sliding_prefix_windows_and_fully_consumed_choices() {
        let original = job(&"é".repeat(PREFIX_BUDGET_CHARS));
        let mut cache = InlineCache::default();
        insert(&mut cache, &original, &["abc".into(), "abc_tail".into()]);
        let mut typed = job(&format!("{}abc", "é".repeat(PREFIX_BUDGET_CHARS - 3)));
        typed.request.snapshot.column = PREFIX_BUDGET_CHARS + 3;
        assert_eq!(cache.get(&typed), Some(vec!["_tail".into()]));
        typed.request.prefix = format!("{}abc_tail", "é".repeat(PREFIX_BUDGET_CHARS - 8));
        typed.request.snapshot.column = PREFIX_BUDGET_CHARS + 8;
        assert!(cache.get(&typed).is_none());
    }

    #[test]
    fn lru_refreshes_hits_and_replaces_same_context() {
        let mut cache = InlineCache::default();
        for id in 0..ENTRY_LIMIT {
            let mut request = job("prefix");
            request.request.snapshot.document_id = DocumentId(id as u64);
            insert(&mut cache, &request, &["answer".into()]);
        }
        let mut oldest = job("prefix");
        oldest.request.snapshot.document_id = DocumentId(0);
        assert!(cache.get(&oldest).is_some());
        let mut newest = oldest.clone();
        newest.request.snapshot.document_id = DocumentId(ENTRY_LIMIT as u64);
        insert(&mut cache, &newest, &["answer".into()]);
        assert_eq!(cache.entries.len(), ENTRY_LIMIT);
        assert!(
            cache.get(&job("prefix")).is_none(),
            "second-oldest document was evicted"
        );
        assert!(cache.get(&oldest).is_some());
        insert(&mut cache, &oldest, &["refreshed".into()]);
        assert_eq!(cache.entries.len(), ENTRY_LIMIT);
        assert_eq!(cache.get(&oldest), Some(vec!["refreshed".into()]));
    }

    #[test]
    fn payload_and_alternative_limits_are_enforced_without_negative_caching() {
        let mut cache = InlineCache::default();
        let request = job("prefix");
        insert(&mut cache, &request, &[]);
        assert!(cache.get(&request).is_none());
        insert(
            &mut cache,
            &request,
            &["x".repeat(PAYLOAD_LIMIT.as_usize() + 1)],
        );
        assert!(cache.entries.is_empty());
        let text = "x".repeat(ByteSize::mebibytes(1).as_usize());
        for id in 0..12 {
            let mut other = request.clone();
            other.request.snapshot.document_id = DocumentId(id);
            insert(&mut cache, &other, std::slice::from_ref(&text));
            assert!(cache.payload_bytes <= PAYLOAD_LIMIT.as_usize());
        }
        assert!(cache.entries.len() < 8);
        insert(
            &mut cache,
            &request,
            &vec!["answer".into(); MAX_ALTERNATIVES + 1],
        );
        assert_eq!(cache.get(&request).unwrap().len(), MAX_ALTERNATIVES);
    }

    #[test]
    fn recency_cache_accounts_for_context_and_rejects_changed_partial_replay() {
        use token::completion::recency::{ContextChunk, CHUNK_BYTES, MAX_CHUNKS};
        let mut original = job("prefix");
        original.request.extra_context = (0..MAX_CHUNKS)
            .map(|index| ContextChunk {
                filename: format!("{index}.rs"),
                text: "x".repeat(CHUNK_BYTES.as_usize()),
            })
            .collect();
        let mut cache = InlineCache::default();
        for id in 0..64 {
            original.request.snapshot.document_id = DocumentId(id);
            insert(&mut cache, &original, &["answer".into()]);
        }
        assert!(cache.payload_bytes <= PAYLOAD_LIMIT.as_usize());
        assert!(
            cache.entries.len() < 32,
            "context payload must evict well before the count limit"
        );
        assert!(cache.get(&original).is_some());
        let mut partial = original.clone();
        partial.request.prefix.push('a');
        partial.request.snapshot.column += 1;
        assert_eq!(cache.get(&partial).unwrap(), ["nswer"]);
        partial.request.extra_context[0].text.push('y');
        assert!(cache.get(&partial).is_none());
        partial.request.extra_context = original.request.extra_context.clone();
        partial.request.extra_context.swap(0, 1);
        assert!(
            cache.get(&partial).is_none(),
            "chunk order is part of the actual prompt"
        );
    }
}
