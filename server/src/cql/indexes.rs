// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! Secondary indexes for efficient CQL query execution.
//!
//! These indexes are built in-memory from the context_metadata_cache at startup
//! and maintained incrementally as new contexts are created.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::store::ContextMetadata;
use crate::turn_store::ContextHead;

/// Secondary indexes for CQL queries.
///
/// Provides O(1) exact match and O(log n) prefix/range queries for indexed fields.
#[derive(Debug, Default)]
pub struct SecondaryIndexes {
    // String field indexes: exact match (HashMap) + sorted for prefix (Vec)
    tag_exact: HashMap<String, HashSet<u64>>,
    tag_sorted: Vec<(String, u64)>,
    tag_lower_exact: HashMap<String, HashSet<u64>>,
    tag_lower_sorted: Vec<(String, u64)>,

    title_exact: HashMap<String, HashSet<u64>>,
    title_sorted: Vec<(String, u64)>,
    title_lower_exact: HashMap<String, HashSet<u64>>,
    title_lower_sorted: Vec<(String, u64)>,

    label_exact: HashMap<String, HashSet<u64>>,

    user_exact: HashMap<String, HashSet<u64>>,
    user_sorted: Vec<(String, u64)>,
    user_lower_exact: HashMap<String, HashSet<u64>>,
    user_lower_sorted: Vec<(String, u64)>,

    service_exact: HashMap<String, HashSet<u64>>,
    service_sorted: Vec<(String, u64)>,
    service_lower_exact: HashMap<String, HashSet<u64>>,
    service_lower_sorted: Vec<(String, u64)>,

    host_exact: HashMap<String, HashSet<u64>>,
    host_sorted: Vec<(String, u64)>,

    trace_id_exact: HashMap<String, HashSet<u64>>,

    // Numeric field indexes
    parent_exact: HashMap<u64, HashSet<u64>>,
    root_exact: HashMap<u64, HashSet<u64>>,

    // Time-based index for range queries
    created_btree: BTreeMap<u64, HashSet<u64>>,

    // Depth index
    depth_btree: BTreeMap<u32, HashSet<u64>>,

    // Track all indexed context IDs for NOT operations
    all_context_ids: HashSet<u64>,
}

impl SecondaryIndexes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build indexes from existing context metadata cache.
    pub fn build_from_cache(
        &mut self,
        metadata_cache: &HashMap<u64, Option<ContextMetadata>>,
        heads: &[ContextHead],
    ) {
        let start = std::time::Instant::now();

        // Index metadata from cache
        for (context_id, metadata_opt) in metadata_cache {
            self.all_context_ids.insert(*context_id);
            if let Some(metadata) = metadata_opt {
                self.index_metadata(*context_id, metadata);
            }
        }

        // Index head data (created_at, depth) from heads
        for head in heads {
            self.all_context_ids.insert(head.context_id);
            self.created_btree
                .entry(head.created_at_unix_ms)
                .or_default()
                .insert(head.context_id);
            self.depth_btree
                .entry(head.head_depth)
                .or_default()
                .insert(head.context_id);
        }

        // Sort the sorted indexes
        self.sort_indexes();

        let elapsed = start.elapsed();
        tracing::info!(
            contexts = self.all_context_ids.len(),
            elapsed_ms = elapsed.as_millis(),
            "Built secondary indexes"
        );
    }

    /// Index a single context's metadata.
    fn index_metadata(&mut self, context_id: u64, metadata: &ContextMetadata) {
        // Tag
        if let Some(tag) = &metadata.client_tag {
            self.tag_exact
                .entry(tag.clone())
                .or_default()
                .insert(context_id);
            self.tag_sorted.push((tag.clone(), context_id));
            let lower = tag.to_lowercase();
            self.tag_lower_exact
                .entry(lower.clone())
                .or_default()
                .insert(context_id);
            self.tag_lower_sorted.push((lower, context_id));
        }

        // Title
        if let Some(title) = &metadata.title {
            self.title_exact
                .entry(title.clone())
                .or_default()
                .insert(context_id);
            self.title_sorted.push((title.clone(), context_id));
            let lower = title.to_lowercase();
            self.title_lower_exact
                .entry(lower.clone())
                .or_default()
                .insert(context_id);
            self.title_lower_sorted.push((lower, context_id));
        }

        // Labels
        if let Some(labels) = &metadata.labels {
            for label in labels {
                self.label_exact
                    .entry(label.clone())
                    .or_default()
                    .insert(context_id);
            }
        }

        // Provenance fields
        if let Some(prov) = &metadata.provenance {
            // User (on_behalf_of)
            if let Some(user) = &prov.on_behalf_of {
                self.user_exact
                    .entry(user.clone())
                    .or_default()
                    .insert(context_id);
                self.user_sorted.push((user.clone(), context_id));
                let lower = user.to_lowercase();
                self.user_lower_exact
                    .entry(lower.clone())
                    .or_default()
                    .insert(context_id);
                self.user_lower_sorted.push((lower, context_id));
            }

            // Service
            if let Some(service) = &prov.service_name {
                self.service_exact
                    .entry(service.clone())
                    .or_default()
                    .insert(context_id);
                self.service_sorted.push((service.clone(), context_id));
                let lower = service.to_lowercase();
                self.service_lower_exact
                    .entry(lower.clone())
                    .or_default()
                    .insert(context_id);
                self.service_lower_sorted.push((lower, context_id));
            }

            // Host
            if let Some(host) = &prov.host_name {
                self.host_exact
                    .entry(host.clone())
                    .or_default()
                    .insert(context_id);
                self.host_sorted.push((host.clone(), context_id));
            }

            // Trace ID
            if let Some(trace_id) = &prov.trace_id {
                self.trace_id_exact
                    .entry(trace_id.clone())
                    .or_default()
                    .insert(context_id);
            }

            // Parent context ID
            if let Some(parent) = prov.parent_context_id {
                self.parent_exact
                    .entry(parent)
                    .or_default()
                    .insert(context_id);
            }

            // Root context ID
            if let Some(root) = prov.root_context_id {
                self.root_exact.entry(root).or_default().insert(context_id);
            }
        }
    }

    /// Sort all sorted indexes for binary search.
    fn sort_indexes(&mut self) {
        self.tag_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.tag_lower_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.title_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.title_lower_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.user_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.user_lower_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.service_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.service_lower_sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.host_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// Add a new context to the indexes.
    pub fn add_context(
        &mut self,
        context_id: u64,
        metadata: Option<&ContextMetadata>,
        created_at_unix_ms: u64,
        depth: u32,
    ) {
        self.all_context_ids.insert(context_id);

        if let Some(metadata) = metadata {
            self.index_metadata(context_id, metadata);
            // Re-sort (expensive, but appends are infrequent compared to queries)
            self.sort_indexes();
        }

        self.created_btree
            .entry(created_at_unix_ms)
            .or_default()
            .insert(context_id);

        self.depth_btree
            .entry(depth)
            .or_default()
            .insert(context_id);
    }

    /// Get all context IDs (for NOT operations).
    pub fn all_contexts(&self) -> &HashSet<u64> {
        &self.all_context_ids
    }

    // =========================================================================
    // Exact match lookups - O(1)
    // =========================================================================

    pub fn lookup_tag_exact(&self, value: &str) -> HashSet<u64> {
        self.tag_exact.get(value).cloned().unwrap_or_default()
    }

    pub fn lookup_tag_exact_ci(&self, value: &str) -> HashSet<u64> {
        self.tag_lower_exact
            .get(&value.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    pub fn lookup_title_exact(&self, value: &str) -> HashSet<u64> {
        self.title_exact.get(value).cloned().unwrap_or_default()
    }

    pub fn lookup_title_exact_ci(&self, value: &str) -> HashSet<u64> {
        self.title_lower_exact
            .get(&value.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    pub fn lookup_label_exact(&self, value: &str) -> HashSet<u64> {
        self.label_exact.get(value).cloned().unwrap_or_default()
    }

    pub fn lookup_user_exact(&self, value: &str) -> HashSet<u64> {
        self.user_exact.get(value).cloned().unwrap_or_default()
    }

    pub fn lookup_user_exact_ci(&self, value: &str) -> HashSet<u64> {
        self.user_lower_exact
            .get(&value.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    pub fn lookup_service_exact(&self, value: &str) -> HashSet<u64> {
        self.service_exact.get(value).cloned().unwrap_or_default()
    }

    pub fn lookup_service_exact_ci(&self, value: &str) -> HashSet<u64> {
        self.service_lower_exact
            .get(&value.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    pub fn lookup_host_exact(&self, value: &str) -> HashSet<u64> {
        self.host_exact.get(value).cloned().unwrap_or_default()
    }

    pub fn lookup_trace_id_exact(&self, value: &str) -> HashSet<u64> {
        self.trace_id_exact.get(value).cloned().unwrap_or_default()
    }

    pub fn lookup_parent_exact(&self, value: u64) -> HashSet<u64> {
        self.parent_exact.get(&value).cloned().unwrap_or_default()
    }

    pub fn lookup_root_exact(&self, value: u64) -> HashSet<u64> {
        self.root_exact.get(&value).cloned().unwrap_or_default()
    }

    // =========================================================================
    // Prefix lookups - O(log n + k) where k is result count
    // =========================================================================

    pub fn lookup_tag_prefix(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.tag_sorted, prefix)
    }

    pub fn lookup_tag_prefix_ci(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.tag_lower_sorted, &prefix.to_lowercase())
    }

    pub fn lookup_title_prefix(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.title_sorted, prefix)
    }

    pub fn lookup_title_prefix_ci(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.title_lower_sorted, &prefix.to_lowercase())
    }

    pub fn lookup_user_prefix(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.user_sorted, prefix)
    }

    pub fn lookup_user_prefix_ci(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.user_lower_sorted, &prefix.to_lowercase())
    }

    pub fn lookup_service_prefix(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.service_sorted, prefix)
    }

    pub fn lookup_service_prefix_ci(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.service_lower_sorted, &prefix.to_lowercase())
    }

    pub fn lookup_host_prefix(&self, prefix: &str) -> HashSet<u64> {
        self.prefix_search(&self.host_sorted, prefix)
    }

    fn prefix_search(&self, sorted: &[(String, u64)], prefix: &str) -> HashSet<u64> {
        if sorted.is_empty() {
            return HashSet::new();
        }

        // Binary search for first element >= prefix
        let start = sorted.partition_point(|(s, _)| s.as_str() < prefix);

        let mut result = HashSet::new();
        for (s, id) in sorted.iter().skip(start) {
            if s.starts_with(prefix) {
                result.insert(*id);
            } else {
                break;
            }
        }
        result
    }

    // =========================================================================
    // Range lookups - O(log n + k)
    // =========================================================================

    pub fn lookup_created_gt(&self, timestamp: u64) -> HashSet<u64> {
        self.created_btree
            .range((
                std::ops::Bound::Excluded(timestamp),
                std::ops::Bound::Unbounded,
            ))
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_created_gte(&self, timestamp: u64) -> HashSet<u64> {
        self.created_btree
            .range(timestamp..)
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_created_lt(&self, timestamp: u64) -> HashSet<u64> {
        self.created_btree
            .range(..timestamp)
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_created_lte(&self, timestamp: u64) -> HashSet<u64> {
        self.created_btree
            .range(..=timestamp)
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_created_eq(&self, timestamp: u64) -> HashSet<u64> {
        self.created_btree
            .get(&timestamp)
            .cloned()
            .unwrap_or_default()
    }

    pub fn lookup_depth_gt(&self, depth: u32) -> HashSet<u64> {
        self.depth_btree
            .range((std::ops::Bound::Excluded(depth), std::ops::Bound::Unbounded))
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_depth_gte(&self, depth: u32) -> HashSet<u64> {
        self.depth_btree
            .range(depth..)
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_depth_lt(&self, depth: u32) -> HashSet<u64> {
        self.depth_btree
            .range(..depth)
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_depth_lte(&self, depth: u32) -> HashSet<u64> {
        self.depth_btree
            .range(..=depth)
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    pub fn lookup_depth_eq(&self, depth: u32) -> HashSet<u64> {
        self.depth_btree.get(&depth).cloned().unwrap_or_default()
    }

    /// Get index statistics.
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            contexts_indexed: self.all_context_ids.len(),
            tag_entries: self.tag_exact.len(),
            title_entries: self.title_exact.len(),
            user_entries: self.user_exact.len(),
            service_entries: self.service_exact.len(),
            host_entries: self.host_exact.len(),
            created_entries: self.created_btree.len(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexStats {
    pub contexts_indexed: usize,
    pub tag_entries: usize,
    pub title_entries: usize,
    pub user_entries: usize,
    pub service_entries: usize,
    pub host_entries: usize,
    pub created_entries: usize,
}
