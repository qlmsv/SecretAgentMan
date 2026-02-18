//! Social media tool for AI-Mentor SaaS platform.
//!
//! Provides content scheduling and posting capabilities for social networks.
//! Currently a stub - OAuth integration to be implemented.

use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

// ══════════════════════════════════════════════════════════════════════════════
// SOCIAL MEDIA TOOL
// ══════════════════════════════════════════════════════════════════════════════

/// Tool for social media content management
pub struct SocialMediaTool {
    /// Supported platforms
    platforms: Vec<&'static str>,
}

impl SocialMediaTool {
    pub fn new() -> Self {
        Self {
            platforms: vec!["linkedin", "twitter", "telegram_channel"],
        }
    }
}

impl Default for SocialMediaTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SocialMediaTool {
    fn name(&self) -> &str {
        "social_media"
    }

    fn description(&self) -> &str {
        "Manage social media content. Actions: list_platforms, generate_post, schedule (stub - OAuth not configured)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list_platforms", "generate_post", "schedule", "status"],
                    "description": "Action to perform"
                },
                "platform": {
                    "type": "string",
                    "enum": ["linkedin", "twitter", "telegram_channel"],
                    "description": "Target platform"
                },
                "content": {
                    "type": "string",
                    "description": "Post content"
                },
                "topic": {
                    "type": "string",
                    "description": "Topic for content generation"
                },
                "tone": {
                    "type": "string",
                    "enum": ["professional", "casual", "educational", "inspirational"],
                    "description": "Tone for generated content"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> anyhow::Result<ToolResult> {
        let action = params
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("list_platforms");

        match action {
            "list_platforms" => {
                let result = json!({
                    "platforms": self.platforms,
                    "connected": [],
                    "note": "OAuth integration pending. Use generate_post for content creation."
                });

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&result).unwrap_or_default(),
                    error: None,
                })
            }

            "generate_post" => {
                let platform = params
                    .get("platform")
                    .and_then(Value::as_str)
                    .unwrap_or("linkedin");
                let topic = params
                    .get("topic")
                    .and_then(Value::as_str)
                    .unwrap_or("productivity");
                let tone = params
                    .get("tone")
                    .and_then(Value::as_str)
                    .unwrap_or("professional");

                // Character limits by platform
                let char_limit = match platform {
                    "twitter" => 280,
                    "linkedin" => 3000,
                    "telegram_channel" => 4096,
                    _ => 1000,
                };

                let result = json!({
                    "platform": platform,
                    "topic": topic,
                    "tone": tone,
                    "char_limit": char_limit,
                    "template": format!(
                        "Generate a {} post about {} for {}. Keep under {} characters.",
                        tone, topic, platform, char_limit
                    ),
                    "hashtag_suggestions": get_hashtags_for_topic(topic),
                    "best_posting_times": get_best_times(platform),
                });

                Ok(ToolResult {
                    success: true,
                    output: serde_json::to_string_pretty(&result).unwrap_or_default(),
                    error: None,
                })
            }

            "schedule" => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "OAuth not configured. Please connect your social accounts first.".to_string(),
                ),
            }),

            "status" => {
                let result = json!({
                    "oauth_configured": false,
                    "connected_accounts": [],
                    "scheduled_posts": 0,
                    "setup_instructions": "OAuth integration requires API keys for each platform. Contact admin."
                });

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

/// Get relevant hashtags for a topic
fn get_hashtags_for_topic(topic: &str) -> Vec<&'static str> {
    match topic.to_lowercase().as_str() {
        "productivity" => vec!["#productivity", "#efficiency", "#workflow", "#tips"],
        "ai" | "artificial intelligence" => vec!["#AI", "#MachineLearning", "#Tech", "#Innovation"],
        "career" => vec!["#career", "#growth", "#professional", "#success"],
        "business" => vec!["#business", "#entrepreneur", "#startup", "#growth"],
        "health" => vec!["#health", "#wellness", "#mindfulness", "#selfcare"],
        _ => vec!["#insights", "#tips", "#growth"],
    }
}

/// Get best posting times for platform
fn get_best_times(platform: &str) -> Vec<&'static str> {
    match platform {
        "linkedin" => vec!["Tuesday 10-11 AM", "Wednesday 12 PM", "Thursday 9-10 AM"],
        "twitter" => vec!["Weekdays 8-10 AM", "12-1 PM", "5-6 PM"],
        "telegram_channel" => vec!["Morning 9-10 AM", "Evening 7-8 PM"],
        _ => vec!["Weekday mornings", "Lunch time"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_social_tool_name() {
        let tool = SocialMediaTool::new();
        assert_eq!(tool.name(), "social_media");
    }

    #[test]
    fn test_social_tool_has_platforms() {
        let tool = SocialMediaTool::new();
        assert!(!tool.platforms.is_empty());
    }

    #[tokio::test]
    async fn test_list_platforms() {
        let tool = SocialMediaTool::new();
        let result = tool
            .execute(json!({
                "action": "list_platforms"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("linkedin"));
    }

    #[tokio::test]
    async fn test_generate_post() {
        let tool = SocialMediaTool::new();
        let result = tool
            .execute(json!({
                "action": "generate_post",
                "platform": "linkedin",
                "topic": "productivity",
                "tone": "professional"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("template"));
    }

    #[tokio::test]
    async fn test_schedule_fails_without_oauth() {
        let tool = SocialMediaTool::new();
        let result = tool
            .execute(json!({
                "action": "schedule",
                "platform": "twitter",
                "content": "Test post"
            }))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error.unwrap().contains("OAuth"));
    }

    #[test]
    fn test_hashtags() {
        let tags = get_hashtags_for_topic("ai");
        assert!(tags.contains(&"#AI"));
    }

    #[test]
    fn test_best_times() {
        let times = get_best_times("linkedin");
        assert!(!times.is_empty());
    }
}
