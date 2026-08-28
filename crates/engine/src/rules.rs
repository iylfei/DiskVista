use crate::{
    application_index::ApplicationIndex, application_origins::OriginKind, safety::SafetyPolicy,
};
use anyhow::Result;
use cleaner_domain::{Assessment, Evidence, FileRecord, InstalledApp, Settings};
use cleaner_platform::{normalize, within};
use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub root: String,
    pub category: String,
    pub owner: String,
    pub age_days: i64,
    pub purpose: String,
    pub consequence: String,
    pub recovery: String,
    #[serde(default)]
    pub warning: String,
    #[serde(default)]
    pub community: bool,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub detect_files: Vec<String>,
}

#[derive(Deserialize)]
struct Pack {
    version: String,
    rules: Vec<Rule>,
}
pub struct RuleSet {
    pub version: String,
    pub rules: Vec<Rule>,
    matchers: Vec<Option<GlobMatcher>>,
    expanded_roots: Vec<Option<String>>,
}

pub fn expand_env(value: &str) -> Option<String> {
    let mut result = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        result.push_str(&rest[..start]);
        let remaining = &rest[start + 1..];
        let end = remaining.find('%')?;
        let key = &remaining[..end];
        result.push_str(&std::env::var(key).ok()?);
        rest = &remaining[end + 1..];
    }
    result.push_str(rest);
    Some(normalize(&result))
}

impl RuleSet {
    pub fn load(community: bool) -> Result<Self> {
        let base: Pack = serde_json::from_str(include_str!("../../../assets/rules/builtin.json"))?;
        let extra: Pack =
            serde_json::from_str(include_str!("../../../assets/rules/community.json"))?;
        let version = format!(
            "{}:{}:{}:classifier-3",
            base.version, extra.version, community
        );
        let mut rules = base.rules;
        if community {
            rules.extend(extra.rules.into_iter().map(|mut r| {
                r.community = true;
                r
            }));
        }
        Ok(Self::compile(version, rules))
    }

    fn compile(version: String, rules: Vec<Rule>) -> Self {
        let expanded_roots: Vec<_> = rules
            .iter()
            .map(|r| {
                if r.detect_files
                    .iter()
                    .any(|d| !expand_env(d).is_some_and(|s| std::path::Path::new(&s).exists()))
                {
                    None
                } else {
                    expand_env(&r.root)
                }
            })
            .collect();
        let matchers = expanded_roots
            .iter()
            .map(|root| {
                root.as_ref()
                    .and_then(|s| {
                        GlobBuilder::new(&s.replace('\\', "/"))
                            .literal_separator(true)
                            .case_insensitive(true)
                            .build()
                            .ok()
                    })
                    .map(|g| g.compile_matcher())
            })
            .collect();
        Self {
            version,
            rules,
            matchers,
            expanded_roots,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_rules(rules: Vec<Rule>) -> Self {
        Self::compile("test".into(), rules)
    }

    pub(crate) fn directory_roots(&self) -> impl Iterator<Item = (&Rule, &str, &GlobMatcher)> {
        self.rules
            .iter()
            .zip(&self.expanded_roots)
            .zip(&self.matchers)
            .filter_map(|((rule, path), matcher)| Some((rule, path.as_deref()?, matcher.as_ref()?)))
    }

    pub(crate) fn matching_rule(&self, normalized_path: &str) -> Option<&Rule> {
        let slash_path = normalized_path.replace('\\', "/");
        self.rules
            .iter()
            .zip(&self.matchers)
            .find_map(|(rule, matcher)| {
                let matches = matcher.as_ref().is_some_and(|m| {
                    std::iter::once(slash_path.as_str())
                        .chain(slash_path.match_indices('/').map(|(i, _)| &slash_path[..i]))
                        .any(|ancestor| m.is_match(ancestor))
                });
                (matches
                    && !rule
                        .excludes
                        .iter()
                        .any(|e| expand_env(e).is_some_and(|root| within(normalized_path, &root))))
                .then_some(rule)
            })
    }

    pub fn classify(
        &self,
        file: &FileRecord,
        policy: &SafetyPolicy,
        apps: &[InstalledApp],
    ) -> Assessment {
        self.classify_indexed(file, policy, &ApplicationIndex::new(apps, policy))
    }

    pub fn classify_indexed(
        &self,
        file: &FileRecord,
        policy: &SafetyPolicy,
        apps: &ApplicationIndex,
    ) -> Assessment {
        let mut a = Assessment {
            category: "unknown".into(),
            owner: None,
            confidence: "low".into(),
            risk: "review".into(),
            purpose: if file.is_dir {
                "尚未识别的目录，需要结合内容和来源判断"
            } else {
                "尚未识别的文件，不能仅凭大小或日期判断无用"
            }
            .into(),
            consequence: "删除可能影响应用功能或丢失个人数据；当前证据不足".into(),
            recovery: "只提供回收站恢复；不保证可重新生成".into(),
            recommendation: "请确认是否仍需要这些文件，可查看判断依据或请求 AI 分析".into(),
            ..Default::default()
        };
        let p = normalize(&file.path);
        if let Some(rule) = self.matching_rule(&p) {
            a.category = rule.category.clone();
            a.owner = Some(rule.owner.clone());
            a.confidence = if rule.community { "medium" } else { "high" }.into();
            a.purpose = rule.purpose.clone();
            a.consequence = rule.consequence.clone();
            a.recovery = rule.recovery.clone();
            a.rule_id = Some(rule.id.clone());
            let changed = file.modified.max(file.latest_change);
            let old = changed > 0
                && chrono::Utc::now().timestamp().saturating_sub(changed) > rule.age_days * 86400;
            a.risk = if old && !rule.community {
                "low"
            } else {
                "review"
            }
            .into();
            a.recommendation = if old {
                "可考虑回收；请确认不再需要并先关闭所属应用"
            } else {
                "近期仍有活动，请先复核用途"
            }
            .into();
            a.evidence.push(Evidence {
                source: if rule.community {
                    "社区规则"
                } else {
                    "内置规则"
                }
                .into(),
                detail: format!("{} · {} · {}", rule.name, rule.id, self.version),
            });
            if !rule.warning.is_empty() {
                a.evidence.push(Evidence {
                    source: "规则警告".into(),
                    detail: rule.warning.clone(),
                });
            }
        }
        if let Some((_, label)) = policy
            .settings
            .labels
            .iter()
            .filter(|(p, _)| within(&file.path, p))
            .max_by_key(|(p, _)| p.len())
        {
            a.owner = Some(label.clone());
            a.confidence = "high".into();
            a.evidence.push(Evidence {
                source: "用户标注".into(),
                detail: label.clone(),
            });
        } else if let Some(origin) = apps.origin(&p) {
            // A specific cleanup rule can be stronger than an inferred data alias.
            // Installation records still identify the enclosing installed product.
            if a.owner.is_none() || origin.kind == OriginKind::Installation {
                a.owner = Some(origin.name.clone());
                a.confidence = origin.confidence.into();
            }
            if a.rule_id.is_none() {
                match origin.kind {
                    OriginKind::Installation => {
                        a.category = "application".into();
                        a.purpose = format!(
                            "{} 的程序安装内容；安装位置关联不等于创建进程记录",
                            origin.name
                        );
                        a.consequence =
                            "直接删除可能导致应用无法启动或组件缺失，请使用应用管理入口".into();
                        a.recovery =
                            "通常需要通过原安装程序修复或重新安装；个人数据应另行备份".into();
                    }
                    OriginKind::ApplicationData => {
                        a.category = "application_data".into();
                        a.purpose = if origin.confidence == "low" {
                            format!(
                                "可能与 {} 关联的数据，依据目录名称推断；用途和创建进程未确认",
                                origin.name
                            )
                        } else {
                            format!("与 {} 关联的应用数据，可能包含配置、缓存及个人内容；未记录创建进程", origin.name)
                        };
                        a.consequence =
                            "删除可能丢失配置、个人内容或影响应用功能；不能将整个数据目录视为缓存"
                                .into();
                    }
                    OriginKind::Portable => {
                        a.category = "application".into();
                        a.purpose = format!("可能是 {} 的应用文件；依据可执行文件布局推断，未确认真实产品或创建进程", origin.name);
                        a.consequence =
                            "可能包含便携程序、模型、配置或安装材料；删除前请确认用途与数据备份"
                                .into();
                    }
                }
            }
            a.evidence.push(origin.evidence.clone());
        } else if a.owner.is_none() {
            let name = file.name.to_lowercase();
            let matches: Vec<_> = if file.is_dir && name.len() > 3 {
                apps.named(&name).collect()
            } else {
                Vec::new()
            };
            if !matches.is_empty() {
                a.owner = Some(
                    matches
                        .iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>()
                        .join(" / "),
                );
                a.confidence = "low".into();
                a.purpose = "目录名与应用名相同，具体用途和真实归属尚待确认".into();
                a.evidence.push(Evidence {
                    source: "名称线索（非创建者证明）".into(),
                    detail: "目录名与应用名相似，可能共享或误匹配".into(),
                });
            }
        }
        if let Some(reason) = policy.reason(file).or_else(|| apps.installed_reason(file)) {
            a.risk = "protected".into();
            a.protected_reason = Some(reason.clone());
            a.recommendation = reason;
        }
        if a.rule_id.is_none() {
            if let Some(info) = crate::directory_knowledge::describe(&file.path) {
                a.category = info.category.into();
                a.purpose = info.purpose.into();
                a.consequence = info.consequence.into();
                a.recovery = info.recovery.into();
                a.owner = None;
                a.confidence = "high".into();
                a.evidence.push(Evidence {
                    source: "Windows 标准目录位置".into(),
                    detail: info.purpose.into(),
                });
            }
        }
        a
    }
    pub fn settings_policy(settings: Settings) -> SafetyPolicy {
        SafetyPolicy::new(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packs_parse() {
        assert!(!RuleSet::load(true).unwrap().rules.is_empty());
    }
    #[test]
    fn deepest_user_label_wins() {
        let mut settings = Settings::default();
        settings
            .labels
            .insert("D:\\records".into(), "parent".into());
        settings
            .labels
            .insert("D:\\records\\app".into(), "specific".into());
        let a = RuleSet::load(false).unwrap().classify(
            &FileRecord {
                path: "D:\\records\\app\\content.txt".into(),
                ..Default::default()
            },
            &SafetyPolicy::new(settings),
            &[],
        );
        assert_eq!(a.owner.as_deref(), Some("specific"));
    }
    #[test]
    fn installing_an_app_at_drive_root_does_not_own_the_drive() {
        let app = InstalledApp {
            id: "node".into(),
            name: "Node.js".into(),
            publisher: String::new(),
            install_location: "D:\\".into(),
            source: "test".into(),
            last_used: None,
        };
        let policy = SafetyPolicy::new(Settings::default());
        let f = FileRecord {
            path: "D:\\personal\\photo.jpg".into(),
            name: "photo.jpg".into(),
            ..Default::default()
        };
        let a = RuleSet::load(false).unwrap().classify(&f, &policy, &[app]);
        assert!(a.owner.is_none());
        assert_ne!(a.risk, "protected");
    }
    #[test]
    fn descendants_inherit_cache_purpose_but_not_safety_exemption() {
        let base = std::env::var("LOCALAPPDATA").unwrap();
        let rules = RuleSet::load(false).unwrap();
        let policy = SafetyPolicy::new(Settings::default());
        let a = rules.classify(
            &FileRecord {
                path: format!("{base}\\Temp\\old.txt"),
                modified: 1,
                ..Default::default()
            },
            &policy,
            &[],
        );
        assert!(a.rule_id.is_some());
        assert_eq!(a.risk, "low");
        let a = rules.classify(
            &FileRecord {
                path: format!("{base}\\Temp\\.env"),
                modified: 1,
                ..Default::default()
            },
            &policy,
            &[],
        );
        assert_eq!(a.risk, "protected");
    }
    #[test]
    fn unknown_is_never_low_risk() {
        let r = RuleSet::load(false).unwrap();
        let a = r.classify(
            &FileRecord {
                path: "D:\\stuff\\strange.bin".into(),
                name: "strange.bin".into(),
                ..Default::default()
            },
            &SafetyPolicy::new(Settings::default()),
            &[],
        );
        assert_eq!(a.risk, "review");
    }
    #[test]
    fn application_purpose_and_protection_are_independent() {
        let app = InstalledApp {
            id: "a".into(),
            name: "Example App".into(),
            publisher: String::new(),
            install_location: "D:\\Apps\\Example".into(),
            source: "test".into(),
            last_used: None,
        };
        let a = RuleSet::load(false).unwrap().classify(
            &FileRecord {
                path: "D:\\Apps\\Example\\data.bin".into(),
                ..Default::default()
            },
            &SafetyPolicy::new(Settings::default()),
            &[app],
        );
        assert_eq!(a.risk, "protected");
        assert_eq!(a.category, "application");
        assert!(!a.purpose.contains("尚未识别"));
    }
    #[test]
    fn program_files_is_a_known_multi_application_container() {
        let path = std::env::var("ProgramFiles(x86)").unwrap();
        let a = RuleSet::load(false).unwrap().classify(
            &FileRecord {
                path,
                is_dir: true,
                ..Default::default()
            },
            &SafetyPolicy::new(Settings::default()),
            &[],
        );
        assert_eq!(a.category, "application_container");
        assert!(a.owner.is_none());
        assert_eq!(a.risk, "protected");
        assert!(a.purpose.contains("多个独立应用"));
    }
    #[test]
    fn protected_wins_over_user_label() {
        let mut s = Settings::default();
        s.labels.insert("C:\\Windows".into(), "我的缓存".into());
        let a = RuleSet::load(true).unwrap().classify(
            &FileRecord {
                path: "C:\\Windows\\a".into(),
                ..Default::default()
            },
            &SafetyPolicy::new(s),
            &[],
        );
        assert_eq!(a.risk, "protected");
    }

    #[test]
    fn app_data_name_evidence_is_inherited_without_lowering_deletion_risk() {
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let app = InstalledApp {
            id: "fixture-editor".into(),
            name: "FixtureEditor".into(),
            publisher: String::new(),
            install_location: "D:\\Installed\\FixtureEditor".into(),
            source: "Windows 卸载清单".into(),
            last_used: None,
        };
        let policy = SafetyPolicy::new(Default::default());
        let index = ApplicationIndex::new(&[app], &policy);
        let rules = RuleSet::load(false).unwrap();
        let f = FileRecord {
            path: format!("{local}\\FixtureEditor\\profiles\\config.json"),
            name: "config.json".into(),
            ..Default::default()
        };
        let assessment = rules.classify_indexed(&f, &policy, &index);
        assert_eq!(assessment.owner.as_deref(), Some("FixtureEditor"));
        assert_eq!(assessment.confidence, "low");
        assert_eq!(assessment.category, "application_data");
        assert_eq!(assessment.risk, "review");
        assert!(assessment.purpose.contains("可能"));
        let sensitive = FileRecord {
            path: format!("{local}\\FixtureEditor\\credentials.json"),
            name: "credentials.json".into(),
            ..Default::default()
        };
        assert_eq!(
            rules.classify_indexed(&sensitive, &policy, &index).risk,
            "protected"
        );
    }

    #[test]
    fn vendor_container_is_not_attributed_to_one_product() {
        let local = std::env::var("LOCALAPPDATA").unwrap();
        let f = FileRecord {
            path: format!("{local}\\Microsoft\\UnrecognizedProduct\\file.bin"),
            ..Default::default()
        };
        let assessment =
            RuleSet::load(false)
                .unwrap()
                .classify(&f, &SafetyPolicy::new(Default::default()), &[]);
        assert!(assessment.owner.is_none());
        assert_ne!(assessment.risk, "low");
    }
}
