//! Esoteric tool for AI-Mentor SaaS platform.
//!
//! Provides calculations for:
//! - Bazi (四柱命理) - Four Pillars of Destiny / Chinese astrology
//! - Destiny Matrix (Матрица Судьбы) - 22 arcana system
//! - MBTI profile storage

use super::traits::{Tool, ToolResult};
use crate::tenant::TenantManager;
use anyhow::{bail, Result};
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

// ══════════════════════════════════════════════════════════════════════════════
// BAZI (四柱命理) - FOUR PILLARS OF DESTINY
// ══════════════════════════════════════════════════════════════════════════════

/// 10 Heavenly Stems (天干)
const HEAVENLY_STEMS: [&str; 10] = [
    "甲", // Jia - Wood Yang
    "乙", // Yi - Wood Yin
    "丙", // Bing - Fire Yang
    "丁", // Ding - Fire Yin
    "戊", // Wu - Earth Yang
    "己", // Ji - Earth Yin
    "庚", // Geng - Metal Yang
    "辛", // Xin - Metal Yin
    "壬", // Ren - Water Yang
    "癸", // Gui - Water Yin
];

/// Heavenly Stems in Pinyin
const HEAVENLY_STEMS_PINYIN: [&str; 10] = [
    "Jia", "Yi", "Bing", "Ding", "Wu", "Ji", "Geng", "Xin", "Ren", "Gui",
];

/// 12 Earthly Branches (地支)
const EARTHLY_BRANCHES: [&str; 12] = [
    "子", // Zi - Rat
    "丑", // Chou - Ox
    "寅", // Yin - Tiger
    "卯", // Mao - Rabbit
    "辰", // Chen - Dragon
    "巳", // Si - Snake
    "午", // Wu - Horse
    "未", // Wei - Goat
    "申", // Shen - Monkey
    "酉", // You - Rooster
    "戌", // Xu - Dog
    "亥", // Hai - Pig
];

/// Earthly Branches in Pinyin
const EARTHLY_BRANCHES_PINYIN: [&str; 12] = [
    "Zi", "Chou", "Yin", "Mao", "Chen", "Si", "Wu", "Wei", "Shen", "You", "Xu", "Hai",
];

/// Chinese zodiac animals
const ZODIAC_ANIMALS: [&str; 12] = [
    "Rat", "Ox", "Tiger", "Rabbit", "Dragon", "Snake", "Horse", "Goat", "Monkey", "Rooster",
    "Dog", "Pig",
];

/// Five elements
const FIVE_ELEMENTS: [&str; 5] = ["Wood", "Fire", "Earth", "Metal", "Water"];

/// A single pillar in Bazi chart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaziPillar {
    pub heavenly_stem: String,
    pub heavenly_stem_pinyin: String,
    pub earthly_branch: String,
    pub earthly_branch_pinyin: String,
    pub element: String,
    pub polarity: String, // Yang or Yin
    pub animal: Option<String>,
}

/// Complete Bazi chart (Four Pillars)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaziChart {
    pub year_pillar: BaziPillar,
    pub month_pillar: BaziPillar,
    pub day_pillar: BaziPillar,
    pub hour_pillar: BaziPillar,
    pub day_master: String,
    pub day_master_element: String,
}

impl BaziChart {
    /// Calculate Bazi chart from birth date and time
    pub fn calculate(date: NaiveDate, time: Option<NaiveTime>) -> Self {
        let year = date.year();
        let month = date.month();
        let _day = date.day(); // Reserved for day pillar calculation

        // Calculate year pillar
        let year_pillar = Self::calculate_year_pillar(year);

        // Calculate month pillar (based on year stem and solar month)
        let month_pillar = Self::calculate_month_pillar(year, month, &year_pillar);

        // Calculate day pillar (60-day cycle)
        let day_pillar = Self::calculate_day_pillar(date);

        // Calculate hour pillar (based on day stem and hour)
        let hour = time.map(|t| t.hour()).unwrap_or(12); // Default to noon
        let hour_pillar = Self::calculate_hour_pillar(hour, &day_pillar);

        let day_master = day_pillar.heavenly_stem_pinyin.clone();
        let day_master_element = day_pillar.element.clone();

        Self {
            year_pillar,
            month_pillar,
            day_pillar,
            hour_pillar,
            day_master,
            day_master_element,
        }
    }

    fn calculate_year_pillar(year: i32) -> BaziPillar {
        // The Chinese year cycle starts from year 4 (2697 BCE as year 1)
        // Using a simpler reference: 1984 was Jia Zi (甲子) year
        let cycle_year = ((year - 1984) % 60 + 60) % 60;
        let stem_index = (cycle_year % 10) as usize;
        let branch_index = (cycle_year % 12) as usize;

        Self::create_pillar(stem_index, branch_index, true)
    }

    fn calculate_month_pillar(_year: i32, month: u32, year_pillar: &BaziPillar) -> BaziPillar {
        // Month branch is fixed: Tiger (寅) = month 1 (Feb), etc.
        // Adjust for Chinese solar months (approximately)
        let branch_index = ((month as i32 + 1) % 12) as usize;

        // Month stem depends on year stem
        // Formula: (year_stem * 2 + month) % 10
        let year_stem_index = HEAVENLY_STEMS_PINYIN
            .iter()
            .position(|&s| s == year_pillar.heavenly_stem_pinyin)
            .unwrap_or(0);
        let stem_index = ((year_stem_index * 2 + month as usize) % 10) as usize;

        Self::create_pillar(stem_index, branch_index, false)
    }

    fn calculate_day_pillar(date: NaiveDate) -> BaziPillar {
        // Reference date: January 1, 1900 was 庚子 (Geng Zi) day
        // That's stem index 6, branch index 0
        let reference = NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
        let days_diff = date.signed_duration_since(reference).num_days();

        // Adjust for the 60-day cycle
        let cycle_day = ((days_diff % 60 + 60) % 60) as usize;
        let stem_index = (cycle_day + 6) % 10; // +6 for Geng
        let branch_index = cycle_day % 12;

        Self::create_pillar(stem_index, branch_index, false)
    }

    fn calculate_hour_pillar(hour: u32, day_pillar: &BaziPillar) -> BaziPillar {
        // Chinese hours are 2-hour blocks starting from 23:00 (Zi hour)
        // 23:00-01:00 = Zi, 01:00-03:00 = Chou, etc.
        let branch_index = if hour == 23 {
            0
        } else {
            ((hour + 1) / 2) as usize % 12
        };

        // Hour stem depends on day stem
        let day_stem_index = HEAVENLY_STEMS_PINYIN
            .iter()
            .position(|&s| s == day_pillar.heavenly_stem_pinyin)
            .unwrap_or(0);

        // Formula: (day_stem % 5) * 2 + hour_branch
        let stem_index = ((day_stem_index % 5) * 2 + branch_index) % 10;

        Self::create_pillar(stem_index, branch_index, false)
    }

    fn create_pillar(stem_index: usize, branch_index: usize, include_animal: bool) -> BaziPillar {
        let element_index = stem_index / 2;
        let polarity = if stem_index % 2 == 0 { "Yang" } else { "Yin" };

        BaziPillar {
            heavenly_stem: HEAVENLY_STEMS[stem_index].to_string(),
            heavenly_stem_pinyin: HEAVENLY_STEMS_PINYIN[stem_index].to_string(),
            earthly_branch: EARTHLY_BRANCHES[branch_index].to_string(),
            earthly_branch_pinyin: EARTHLY_BRANCHES_PINYIN[branch_index].to_string(),
            element: FIVE_ELEMENTS[element_index].to_string(),
            polarity: polarity.to_string(),
            animal: if include_animal {
                Some(ZODIAC_ANIMALS[branch_index].to_string())
            } else {
                None
            },
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// DESTINY MATRIX (МАТРИЦА СУДЬБЫ) - 22 ARCANA SYSTEM
// ══════════════════════════════════════════════════════════════════════════════

/// Destiny Matrix calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinyMatrix {
    /// Core number (1-22)
    pub core_number: u32,
    /// Arcana name
    pub arcana_name: String,
    /// Arcana description
    pub arcana_description: String,
    /// Day number
    pub day_number: u32,
    /// Month number
    pub month_number: u32,
    /// Year number
    pub year_number: u32,
    /// Life path number
    pub life_path: u32,
}

/// Names of 22 Major Arcana
const ARCANA_NAMES: [&str; 22] = [
    "The Magician",       // 1
    "The High Priestess", // 2
    "The Empress",        // 3
    "The Emperor",        // 4
    "The Hierophant",     // 5
    "The Lovers",         // 6
    "The Chariot",        // 7
    "Strength",           // 8
    "The Hermit",         // 9
    "Wheel of Fortune",   // 10
    "Justice",            // 11
    "The Hanged Man",     // 12
    "Death",              // 13
    "Temperance",         // 14
    "The Devil",          // 15
    "The Tower",          // 16
    "The Star",           // 17
    "The Moon",           // 18
    "The Sun",            // 19
    "Judgement",          // 20
    "The World",          // 21
    "The Fool",           // 22
];

/// Brief descriptions of 22 Major Arcana
const ARCANA_DESCRIPTIONS: [&str; 22] = [
    "Willpower, skill, resourcefulness, creativity",                            // 1
    "Intuition, mystery, inner knowledge, patience",                            // 2
    "Abundance, nurturing, creativity, nature",                                 // 3
    "Authority, structure, stability, leadership",                              // 4
    "Tradition, conformity, spiritual wisdom, guidance",                        // 5
    "Love, harmony, relationships, choices",                                    // 6
    "Determination, willpower, success, control",                               // 7
    "Inner strength, courage, patience, compassion",                            // 8
    "Soul-searching, introspection, wisdom, solitude",                          // 9
    "Change, cycles, destiny, turning points",                                  // 10
    "Fairness, truth, cause and effect, law",                                   // 11
    "Sacrifice, letting go, new perspective, pause",                            // 12
    "Transformation, endings, change, transition",                              // 13
    "Balance, moderation, patience, purpose",                                   // 14
    "Bondage, materialism, temptation, shadow self",                            // 15
    "Sudden change, upheaval, revelation, awakening",                           // 16
    "Hope, inspiration, renewal, serenity",                                     // 17
    "Illusion, fear, anxiety, subconscious",                                    // 18
    "Joy, success, vitality, positivity",                                       // 19
    "Reflection, reckoning, awakening, renewal",                                // 20
    "Completion, integration, accomplishment, wholeness",                       // 21
    "New beginnings, innocence, spontaneity, free spirit",                      // 22
];

impl DestinyMatrix {
    /// Calculate Destiny Matrix from birth date
    pub fn calculate(date: NaiveDate) -> Self {
        let day = date.day();
        let month = date.month();
        let year = date.year() as u32;

        // Reduce day to 1-22
        let day_number = Self::reduce_to_arcana(day);

        // Reduce month to 1-22
        let month_number = Self::reduce_to_arcana(month);

        // Reduce year to 1-22
        let year_digits_sum: u32 = year
            .to_string()
            .chars()
            .filter_map(|c| c.to_digit(10))
            .sum();
        let year_number = Self::reduce_to_arcana(year_digits_sum);

        // Calculate core number (sum of all)
        let total = day_number + month_number + year_number;
        let core_number = Self::reduce_to_arcana(total);

        // Life path is the sum of all original digits
        let all_digits: u32 = format!("{:02}{:02}{}", day, month, year)
            .chars()
            .filter_map(|c| c.to_digit(10))
            .sum();
        let life_path = Self::reduce_to_arcana(all_digits);

        let arcana_index = (core_number - 1) as usize;

        Self {
            core_number,
            arcana_name: ARCANA_NAMES[arcana_index].to_string(),
            arcana_description: ARCANA_DESCRIPTIONS[arcana_index].to_string(),
            day_number,
            month_number,
            year_number,
            life_path,
        }
    }

    /// Reduce a number to 1-22 range
    fn reduce_to_arcana(mut num: u32) -> u32 {
        while num > 22 {
            num = num
                .to_string()
                .chars()
                .filter_map(|c| c.to_digit(10))
                .sum();
        }
        if num == 0 {
            num = 22; // 0 maps to The Fool (22)
        }
        num
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// ESOTERIC TOOL
// ══════════════════════════════════════════════════════════════════════════════

/// Esoteric tool for Bazi, Destiny Matrix, and MBTI
pub struct EsotericTool {
    tenant_manager: Option<Arc<TenantManager>>,
}

impl EsotericTool {
    pub fn new(tenant_manager: Option<Arc<TenantManager>>) -> Self {
        Self { tenant_manager }
    }

    fn parse_date(date_str: &str) -> Result<NaiveDate> {
        // Try multiple formats
        if let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            return Ok(d);
        }
        if let Ok(d) = NaiveDate::parse_from_str(date_str, "%d.%m.%Y") {
            return Ok(d);
        }
        if let Ok(d) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y") {
            return Ok(d);
        }
        bail!("Invalid date format. Use YYYY-MM-DD, DD.MM.YYYY, or DD/MM/YYYY")
    }

    fn parse_time(time_str: &str) -> Result<NaiveTime> {
        if let Ok(t) = NaiveTime::parse_from_str(time_str, "%H:%M") {
            return Ok(t);
        }
        if let Ok(t) = NaiveTime::parse_from_str(time_str, "%H:%M:%S") {
            return Ok(t);
        }
        bail!("Invalid time format. Use HH:MM or HH:MM:SS")
    }
}

#[async_trait]
impl Tool for EsotericTool {
    fn name(&self) -> &str {
        "esoteric"
    }

    fn description(&self) -> &str {
        "Calculate Bazi (Four Pillars of Destiny), Destiny Matrix (22 Arcana), or store MBTI type. \
         Actions: bazi_calculate, destiny_matrix, mbti_store, mbti_get"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["bazi_calculate", "destiny_matrix", "mbti_store", "mbti_get"],
                    "description": "The action to perform"
                },
                "birthdate": {
                    "type": "string",
                    "description": "Birth date in YYYY-MM-DD, DD.MM.YYYY, or DD/MM/YYYY format"
                },
                "birth_time": {
                    "type": "string",
                    "description": "Birth time in HH:MM format (for Bazi calculation)"
                },
                "mbti_type": {
                    "type": "string",
                    "description": "MBTI personality type (e.g., INTJ, ENFP) for mbti_store action"
                },
                "user_id": {
                    "type": "string",
                    "description": "User ID for storing/retrieving MBTI type"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' parameter"))?;

        match action {
            "bazi_calculate" => {
                let birthdate = args
                    .get("birthdate")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'birthdate' for Bazi calculation"))?;

                let date = Self::parse_date(birthdate)?;
                let time = args
                    .get("birth_time")
                    .and_then(|v| v.as_str())
                    .and_then(|t| Self::parse_time(t).ok());

                let chart = BaziChart::calculate(date, time);

                let output = format!(
                    "Bazi Chart (Four Pillars of Destiny):\n\n\
                     Year Pillar: {} {} ({} {}) - {} {}\n\
                     Month Pillar: {} {} ({} {})\n\
                     Day Pillar: {} {} ({} {}) - Day Master\n\
                     Hour Pillar: {} {} ({} {})\n\n\
                     Day Master: {} ({})\n\
                     Chinese Zodiac: {}",
                    chart.year_pillar.heavenly_stem,
                    chart.year_pillar.earthly_branch,
                    chart.year_pillar.heavenly_stem_pinyin,
                    chart.year_pillar.earthly_branch_pinyin,
                    chart.year_pillar.element,
                    chart.year_pillar.polarity,
                    chart.month_pillar.heavenly_stem,
                    chart.month_pillar.earthly_branch,
                    chart.month_pillar.heavenly_stem_pinyin,
                    chart.month_pillar.earthly_branch_pinyin,
                    chart.day_pillar.heavenly_stem,
                    chart.day_pillar.earthly_branch,
                    chart.day_pillar.heavenly_stem_pinyin,
                    chart.day_pillar.earthly_branch_pinyin,
                    chart.hour_pillar.heavenly_stem,
                    chart.hour_pillar.earthly_branch,
                    chart.hour_pillar.heavenly_stem_pinyin,
                    chart.hour_pillar.earthly_branch_pinyin,
                    chart.day_master,
                    chart.day_master_element,
                    chart.year_pillar.animal.as_deref().unwrap_or("Unknown")
                );

                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }

            "destiny_matrix" => {
                let birthdate = args
                    .get("birthdate")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'birthdate' for Destiny Matrix"))?;

                let date = Self::parse_date(birthdate)?;
                let matrix = DestinyMatrix::calculate(date);

                let output = format!(
                    "Destiny Matrix (22 Arcana System):\n\n\
                     Core Number: {} - {}\n\
                     Description: {}\n\n\
                     Components:\n\
                     - Day Number: {}\n\
                     - Month Number: {}\n\
                     - Year Number: {}\n\
                     - Life Path: {}",
                    matrix.core_number,
                    matrix.arcana_name,
                    matrix.arcana_description,
                    matrix.day_number,
                    matrix.month_number,
                    matrix.year_number,
                    matrix.life_path
                );

                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }

            "mbti_store" => {
                let mbti_type = args
                    .get("mbti_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'mbti_type' parameter"))?;

                // Validate MBTI type
                let mbti_upper = mbti_type.to_uppercase();
                if mbti_upper.len() != 4 {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some("MBTI type must be exactly 4 letters (e.g., INTJ)".to_string()),
                    });
                }

                let valid_chars = [
                    ['I', 'E'],
                    ['N', 'S'],
                    ['T', 'F'],
                    ['J', 'P'],
                ];
                let chars: Vec<char> = mbti_upper.chars().collect();

                for (i, &c) in chars.iter().enumerate() {
                    if !valid_chars[i].contains(&c) {
                        return Ok(ToolResult {
                            success: false,
                            output: String::new(),
                            error: Some(format!(
                                "Invalid MBTI type. Position {} must be {} or {}",
                                i + 1,
                                valid_chars[i][0],
                                valid_chars[i][1]
                            )),
                        });
                    }
                }

                // Store in tenant DB if available
                if let Some(ref tenant_manager) = self.tenant_manager {
                    if let Some(user_id) = args.get("user_id").and_then(|v| v.as_str()) {
                        if let Ok(tenant) = tenant_manager.get_tenant(user_id) {
                            tenant.set_profile_value("mbti_type", &mbti_upper)?;
                        }
                    }
                }

                Ok(ToolResult {
                    success: true,
                    output: format!("MBTI type {} stored successfully", mbti_upper),
                    error: None,
                })
            }

            "mbti_get" => {
                if let Some(ref tenant_manager) = self.tenant_manager {
                    if let Some(user_id) = args.get("user_id").and_then(|v| v.as_str()) {
                        if let Ok(tenant) = tenant_manager.get_tenant(user_id) {
                            if let Ok(Some(mbti)) = tenant.get_profile_value("mbti_type") {
                                return Ok(ToolResult {
                                    success: true,
                                    output: format!("MBTI type: {}", mbti),
                                    error: None,
                                });
                            }
                        }
                    }
                }

                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some("No MBTI type stored for this user".to_string()),
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
    fn test_destiny_matrix_reduction() {
        // Test basic reduction
        assert_eq!(DestinyMatrix::reduce_to_arcana(5), 5);
        assert_eq!(DestinyMatrix::reduce_to_arcana(22), 22);
        assert_eq!(DestinyMatrix::reduce_to_arcana(23), 5); // 2+3=5
        assert_eq!(DestinyMatrix::reduce_to_arcana(25), 7); // 2+5=7
        assert_eq!(DestinyMatrix::reduce_to_arcana(99), 18); // 9+9=18 (valid arcana 1-22)
    }

    #[test]
    fn test_destiny_matrix_calculation() {
        let date = NaiveDate::from_ymd_opt(1990, 5, 15).unwrap();
        let matrix = DestinyMatrix::calculate(date);

        assert!(matrix.core_number >= 1 && matrix.core_number <= 22);
        assert!(matrix.day_number >= 1 && matrix.day_number <= 22);
        assert!(matrix.month_number >= 1 && matrix.month_number <= 22);
        assert!(matrix.year_number >= 1 && matrix.year_number <= 22);
    }

    #[test]
    fn test_bazi_year_pillar() {
        // 1984 should be Jia Zi (甲子)
        let pillar = BaziChart::calculate_year_pillar(1984);
        assert_eq!(pillar.heavenly_stem_pinyin, "Jia");
        assert_eq!(pillar.earthly_branch_pinyin, "Zi");
        assert_eq!(pillar.animal, Some("Rat".to_string()));

        // 1985 should be Yi Chou (乙丑)
        let pillar = BaziChart::calculate_year_pillar(1985);
        assert_eq!(pillar.heavenly_stem_pinyin, "Yi");
        assert_eq!(pillar.earthly_branch_pinyin, "Chou");
        assert_eq!(pillar.animal, Some("Ox".to_string()));
    }

    #[test]
    fn test_bazi_full_chart() {
        let date = NaiveDate::from_ymd_opt(1990, 6, 15).unwrap();
        let time = NaiveTime::from_hms_opt(14, 30, 0);
        let chart = BaziChart::calculate(date, time);

        // Verify all pillars are populated
        assert!(!chart.year_pillar.heavenly_stem.is_empty());
        assert!(!chart.month_pillar.heavenly_stem.is_empty());
        assert!(!chart.day_pillar.heavenly_stem.is_empty());
        assert!(!chart.hour_pillar.heavenly_stem.is_empty());

        // Year pillar should have animal
        assert!(chart.year_pillar.animal.is_some());
    }

    #[test]
    fn test_date_parsing() {
        assert!(EsotericTool::parse_date("1990-06-15").is_ok());
        assert!(EsotericTool::parse_date("15.06.1990").is_ok());
        assert!(EsotericTool::parse_date("15/06/1990").is_ok());
        assert!(EsotericTool::parse_date("invalid").is_err());
    }

    #[tokio::test]
    async fn test_esoteric_tool_destiny_matrix() {
        let tool = EsotericTool::new(None);

        let result = tool
            .execute(json!({
                "action": "destiny_matrix",
                "birthdate": "1990-05-15"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("Destiny Matrix"));
        assert!(result.output.contains("Core Number"));
    }

    #[tokio::test]
    async fn test_esoteric_tool_bazi() {
        let tool = EsotericTool::new(None);

        let result = tool
            .execute(json!({
                "action": "bazi_calculate",
                "birthdate": "1990-05-15",
                "birth_time": "14:30"
            }))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.output.contains("Bazi Chart"));
        assert!(result.output.contains("Day Master"));
    }

    #[tokio::test]
    async fn test_mbti_validation() {
        let tool = EsotericTool::new(None);

        // Valid MBTI
        let result = tool
            .execute(json!({
                "action": "mbti_store",
                "mbti_type": "INTJ"
            }))
            .await
            .unwrap();
        assert!(result.success);

        // Invalid MBTI
        let result = tool
            .execute(json!({
                "action": "mbti_store",
                "mbti_type": "XXXX"
            }))
            .await
            .unwrap();
        assert!(!result.success);
    }
}
