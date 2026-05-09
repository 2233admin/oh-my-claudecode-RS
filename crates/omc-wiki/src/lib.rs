//! omc-wiki: Wiki knowledge layer for oh-my-claudecode-RS
//!
//! Persistent, self-maintained markdown knowledge base that compounds
//! project and session knowledge across sessions.
//!
//! Provides 7 MCP tools: wiki_ingest, wiki_query, wiki_lint,
//! wiki_add, wiki_list, wiki_read, wiki_delete.

pub mod storage;
pub mod wiki;

pub use storage::{
    WikiError, append_log, delete_page, ensure_wiki_dir, get_wiki_dir, list_pages, read_all_pages,
    read_index, read_log, read_page, title_to_slug, write_page,
};
pub use wiki::{
    WIKI_CATEGORIES, WIKI_SCHEMA_VERSION, WikiConfig, WikiIngestInput, WikiIngestResult,
    WikiLintIssue, WikiLintReport, WikiLintSeverity, WikiLogEntry, WikiPage, WikiPageFrontmatter,
    WikiQueryMatch, WikiQueryOptions, ingest_knowledge, lint_wiki, query_wiki,
};
