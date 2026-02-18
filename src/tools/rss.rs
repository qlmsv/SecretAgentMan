//! RSS feed tool for AI-Mentor SaaS platform.
//!
//! Provides RSS feed fetching and aggregation capabilities.

use super::traits::{Tool, ToolResult};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ══════════════════════════════════════════════════════════════════════════════
// RSS FEED TYPES
// ══════════════════════════════════════════════════════════════════════════════

/// A single RSS feed item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub title: String,
    pub link: String,
    pub description: Option<String>,
    pub pub_date: Option<String>,
    pub source: String,
}

/// Aggregated feed result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedResult {
    pub items: Vec<FeedItem>,
    pub total_count: usize,
    pub sources_fetched: usize,
}

// ══════════════════════════════════════════════════════════════════════════════
// RSS TOOL
// ══════════════════════════════════════════════════════════════════════════════

/// Tool for fetching and aggregating RSS feeds
pub struct RssTool {
    client: reqwest::Client,
    default_feeds: Vec<String>,
}

impl RssTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            default_feeds: vec![
                // Tech news
                "https://news.ycombinator.com/rss".to_string(),
                "https://feeds.feedburner.com/TechCrunch".to_string(),
                // AI/ML
                "https://openai.com/blog/rss.xml".to_string(),
            ],
        }
    }

    /// Fetch a single RSS feed
    async fn fetch_feed(&self, url: &str) -> Result<Vec<FeedItem>> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "ZeroClaw/1.0 RSS Reader")
            .send()
            .await
            .context("Failed to fetch RSS feed")?;

        let bytes = response.bytes().await.context("Failed to read response")?;
        let channel = rss::Channel::read_from(&bytes[..]).context("Failed to parse RSS feed")?;

        let source_name = channel.title().to_string();
        let items: Vec<FeedItem> = channel
            .items()
            .iter()
            .take(10) // Limit items per feed
            .map(|item| FeedItem {
                title: item.title().unwrap_or("Untitled").to_string(),
                link: item.link().unwrap_or("").to_string(),
                description: item.description().map(|d| {
                    // Strip HTML tags for cleaner output
                    strip_html_tags(d)
                }),
                pub_date: item.pub_date().map(|d| d.to_string()),
                source: source_name.clone(),
            })
            .collect();

        Ok(items)
    }

    /// Fetch multiple feeds and aggregate results
    async fn fetch_feeds(&self, urls: &[String], limit: usize) -> FeedResult {
        let mut all_items = Vec::new();
        let mut sources_fetched = 0;

        for url in urls {
            match self.fetch_feed(url).await {
                Ok(items) => {
                    all_items.extend(items);
                    sources_fetched += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch feed {}: {}", url, e);
                }
            }
        }

        // Sort by publication date (newest first)
        all_items.sort_by(|a, b| {
            b.pub_date
                .as_ref()
                .unwrap_or(&String::new())
                .cmp(a.pub_date.as_ref().unwrap_or(&String::new()))
        });

        let total_count = all_items.len();
        all_items.truncate(limit);

        FeedResult {
            items: all_items,
            total_count,
            sources_fetched,
        }
    }
}

impl Default for RssTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip HTML tags from a string
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;

    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }

    // Clean up whitespace
    result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

#[async_trait]
impl Tool for RssTool {
    fn name(&self) -> &str {
        "rss"
    }

    fn description(&self) -> &str {
        "Fetch and aggregate RSS feeds. Actions: fetch (get items from feeds), list_defaults (show default feeds)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["fetch", "list_defaults"],
                    "description": "Action to perform"
                },
                "feeds": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "RSS feed URLs to fetch (optional, uses defaults if not provided)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of items to return (default: 20)"
                },
                "category": {
                    "type": "string",
                    "enum": ["tech", "ai", "business", "all"],
                    "description": "Category filter for default feeds"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> anyhow::Result<ToolResult> {
        let action = params
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("fetch");

        match action {
            "list_defaults" => {
                let feeds_json = json!({
                    "default_feeds": self.default_feeds,
                    "categories": {
                        "tech": ["https://news.ycombinator.com/rss", "https://feeds.feedburner.com/TechCrunch"],
                        "ai": ["https://openai.com/blog/rss.xml"],
                        "business": ["https://feeds.bloomberg.com/markets/news.rss"]
                    }
                });

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&feeds_json).unwrap_or_default(),
                    error: None,
                })
            }

            "fetch" => {
                let limit = params
                    .get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(20) as usize;

                let feeds: Vec<String> = if let Some(feeds_arr) = params.get("feeds") {
                    serde_json::from_value(feeds_arr.clone()).unwrap_or_default()
                } else {
                    // Use category or defaults
                    let category = params
                        .get("category")
                        .and_then(Value::as_str)
                        .unwrap_or("all");

                    match category {
                        "tech" => vec![
                            "https://news.ycombinator.com/rss".to_string(),
                            "https://feeds.feedburner.com/TechCrunch".to_string(),
                        ],
                        "ai" => vec!["https://openai.com/blog/rss.xml".to_string()],
                        "business" => {
                            vec!["https://feeds.bloomberg.com/markets/news.rss".to_string()]
                        }
                        _ => self.default_feeds.clone(),
                    }
                };

                if feeds.is_empty() {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("No feeds provided".to_string()),
                    });
                }

                let result = self.fetch_feeds(&feeds, limit).await;

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&result).unwrap_or_default(),
                    error: None,
                })
            }

            _ => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Unknown action: {}", action)),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        let html = "<p>Hello <strong>World</strong>!</p>";
        assert_eq!(strip_html_tags(html), "Hello World!");
    }

    #[test]
    fn test_strip_html_with_entities() {
        let html = "<div>Test &amp; more</div>";
        let result = strip_html_tags(html);
        assert!(result.contains("Test"));
        assert!(result.contains("&amp;"));
    }

    #[test]
    fn test_rss_tool_name() {
        let tool = RssTool::new();
        assert_eq!(tool.name(), "rss");
    }

    #[test]
    fn test_rss_tool_has_description() {
        let tool = RssTool::new();
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_rss_tool_schema() {
        let tool = RssTool::new();
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["action"].is_object());
        assert!(schema["properties"]["feeds"].is_object());
    }

    #[tokio::test]
    async fn test_list_defaults() {
        let tool = RssTool::new();
        let result = tool
            .execute(json!({
                "action": "list_defaults"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("default_feeds"));
    }

    #[test]
    fn test_feed_item_serialization() {
        let item = FeedItem {
            title: "Test Article".to_string(),
            link: "https://example.com/article".to_string(),
            description: Some("Test description".to_string()),
            pub_date: Some("2024-01-01".to_string()),
            source: "Test Source".to_string(),
        };

        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("Test Article"));
        assert!(json.contains("Test Source"));
    }
}
