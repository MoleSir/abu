use std::{
    collections::HashMap,
    path::PathBuf,
};

use abu_tool::ToolCategory;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ===================================================================== //
//                  Base data
// ===================================================================== //

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Behavior {
    Allow,
    Deny,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ExecutionMode {
    #[default]
    Default,
    Plan,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UserResponse {
    Yes,
    No,
    Always,
}

#[derive(Debug)]
pub struct PermissionResult {
    pub behavior: Behavior,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub enum Matcher {
    Any,
    Exact(Value),
    Contains(String),
    Regex(Regex),
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub tool: String,
    pub arg_patterns: HashMap<String, Matcher>,
    pub behavior: Behavior,
}

#[async_trait::async_trait]
pub trait UserAuthorizer: Send + Sync {
    async fn ask_user(&self, tool_name: &str, arguments: &Value, preview_reason: &str) -> UserResponse;
}

// ===================================================================== //
//                  Permission Manager
// ===================================================================== //

/// Serializable representation of a persisted allow/deny rule.
#[derive(Serialize, Deserialize)]
struct PersistedRule {
    tool: String,
    behavior: String,
}

#[derive(Serialize, Deserialize)]
struct PersistedPermissions {
    rules: Vec<PersistedRule>,
}

pub struct PermissionManager {
    pub mode: ExecutionMode,
    rules: Vec<Rule>,
    authorizer: Box<dyn UserAuthorizer>,
    permissions_file: Option<PathBuf>,
}

impl PermissionManager {
    pub fn new(mode: ExecutionMode, authorizer: impl UserAuthorizer + 'static) -> Self {
        Self {
            mode,
            rules: vec![],
            authorizer: Box::new(authorizer),
            permissions_file: None,
        }
    }

    /// Set the file path where "always allow" rules are persisted.
    pub fn with_permissions_file(mut self, path: PathBuf) -> Self {
        self.permissions_file = Some(path);
        self
    }

    /// Load previously persisted rules from disk.
    pub fn load_persisted_rules(&mut self) -> Result<(), String> {
        let Some(ref path) = self.permissions_file else {
            return Ok(());
        };
        if !path.exists() {
            return Ok(());
        }
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let persisted: PersistedPermissions =
            serde_json::from_str(&content).map_err(|e| e.to_string())?;

        for pr in persisted.rules {
            let behavior = match pr.behavior.as_str() {
                "allow" => Behavior::Allow,
                "deny" => Behavior::Deny,
                _ => continue,
            };
            self.rules.push(Rule {
                tool: pr.tool,
                arg_patterns: HashMap::new(),
                behavior,
            });
        }

        Ok(())
    }

    /// Append a single rule to the persisted rules file.
    fn persist_rule(&self, tool_name: &str, behavior: Behavior) {
        let Some(ref path) = self.permissions_file else {
            return;
        };

        let behavior_str = match behavior {
            Behavior::Allow => "allow",
            Behavior::Deny => "deny",
            Behavior::Ask => return, // Ask rules are not persisted
        };

        // Read existing rules, append new one, deduplicate by tool name
        let mut persisted = if path.exists() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str::<PersistedPermissions>(&s).ok())
                .unwrap_or(PersistedPermissions { rules: vec![] })
        } else {
            PersistedPermissions { rules: vec![] }
        };

        // Remove existing entry for the same tool (update in place)
        persisted.rules.retain(|r| r.tool != tool_name);
        persisted.rules.push(PersistedRule {
            tool: tool_name.to_string(),
            behavior: behavior_str.to_string(),
        });

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, serde_json::to_string_pretty(&persisted).unwrap_or_default());
    }   

    #[inline]
    pub fn with_deny(self, tool: impl Into<String>) -> Self {
        self.with_rule_behavior(tool, Behavior::Deny)
    }

    #[inline]
    pub fn with_deny_if(self, tool: impl Into<String>, arg: impl Into<String>, matcher: Matcher) -> Self {
        self.with_rule_behavior_if(tool, arg, matcher, Behavior::Deny)
    }

    #[inline]
    pub fn with_allow(self, tool: impl Into<String>) -> Self {
        self.with_rule_behavior(tool, Behavior::Allow)
    }

    #[inline]
    pub fn with_allow_if(self, tool: impl Into<String>, arg: impl Into<String>, matcher: Matcher) -> Self {
        self.with_rule_behavior_if(tool, arg, matcher, Behavior::Allow)
    }

    #[inline]
    pub fn with_ask(self, tool: impl Into<String>) -> Self {
        self.with_rule_behavior(tool, Behavior::Ask)
    }

    #[inline]
    pub fn with_ask_if(self, tool: impl Into<String>, arg: impl Into<String>, matcher: Matcher) -> Self {
        self.with_rule_behavior_if(tool, arg, matcher, Behavior::Ask)
    }
    
    pub fn with_rule_behavior(mut self, tool: impl Into<String>, behavior: Behavior) -> Self {
        self.rules.push(Rule {
            tool: tool.into(),
            arg_patterns: HashMap::new(),
            behavior,
        });
        self
    }

    pub fn with_rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn with_rule_behavior_if(mut self, tool: impl Into<String>, arg: impl Into<String>, matcher: Matcher, behavior: Behavior) -> Self {
        let mut patterns = HashMap::new();
        patterns.insert(arg.into(), matcher);
        self.rules.push(Rule {
            tool: tool.into(),
            arg_patterns: patterns,
            behavior,
        });
        self
    }

    pub fn check(&self, category: ToolCategory, tool_name: &str, args: &Value) -> PermissionResult {
        // 1. Deny Rules (绝对拒绝规则优先)
        for rule in self.rules.iter().filter(|r| r.behavior == Behavior::Deny) {
            if self.matches_rule(rule, tool_name, args) {
                return PermissionResult {
                    behavior: Behavior::Deny,
                    reason: "Blocked by explicit deny rule".to_string(),
                };
            }
        }

        // 2. 模式检查（到这里的 rule 已经排除了明确拒绝的规则），根据模式直接拒绝/同意一些操作
        match self.mode {
            // plan 模式自动拒绝写操作，同意读操作
            ExecutionMode::Plan => {
                match category {
                    ToolCategory::Mutating => return PermissionResult {
                        behavior: Behavior::Deny,
                        reason: "Plan mode: write operations are blocked".to_string(),
                    },
                    ToolCategory::Safe => return PermissionResult {
                        behavior: Behavior::Allow,
                        reason: "Plan mode: read-only allowed".to_string(),
                    }
                }
            }
            // auto 模型允许
            ExecutionMode::Auto => {
                if let ToolCategory::Safe = category {
                    return PermissionResult {
                        behavior: Behavior::Allow,
                        reason: "Auto mode: read-only tool auto-approved".to_string(),
                    };
                }
            }
            // 默认模式下不做处理
            ExecutionMode::Default => {}
        }        

        // 3. Allow Rules：检查工具调用是否满足允许操作，如果满足直接返回
        for rule in self.rules.iter().filter(|r| r.behavior == Behavior::Allow) {
            if self.matches_rule(rule, tool_name, args) {
                return PermissionResult {
                    behavior: Behavior::Allow,
                    reason: "Matched explicit allow rule".to_string(),
                };
            }
        }

        // 4. 指定的操作不直接允许也不直接拒绝，返回给用户判定
        PermissionResult {
            behavior: Behavior::Ask,
            reason: format!("No rule matched for {}, asking user", tool_name),
        }
    }

    /// 权限检查：
    /// 1. 将调用与用户明确拒绝的操作进行比较，如果满足直接拒绝
    /// 2. 根据当前模式，自动处理一些权限：
    ///   - Plan 模式：拒绝所有写操作、同意所有读操作
    ///   - Auto 模式：同意所有读操作，不处理写操作
    ///   - Default 模式：不处理
    /// 3. 将调用与用户明确同意的操作进行比较，如果满足直接同意
    /// 4. 剩下的操作可能是：1. 用户没有明确同意/拒绝、2. 用户明确需要确认操作，需要等待用户的操作
    pub async fn request_permission(&mut self, category: ToolCategory, tool_name: &str, args: &Value) -> Result<String, String> {
        let decision = self.check(category, tool_name, args);

        match decision.behavior {
            Behavior::Deny => Err(format!("Permission denied: {}", decision.reason)),
            Behavior::Allow => Ok("Allowed".to_string()),
            Behavior::Ask => {
                let user_res = self.authorizer.ask_user(tool_name, args, &decision.reason).await;
                match user_res {
                    UserResponse::Always => {
                        let rule = Rule {
                            tool: tool_name.to_string(),
                            arg_patterns: HashMap::new(),
                            behavior: Behavior::Allow,
                        };
                        self.persist_rule(tool_name, Behavior::Allow);
                        self.rules.push(rule);
                        Ok("Allowed by user (rule saved)".to_string())
                    }
                    UserResponse::Yes => {
                        Ok("Allowed by user".to_string())
                    }
                    UserResponse::No => {
                        Err("Permission denied by user".to_string())
                    }
                }
            }
        }
    }

    fn matches_rule(&self, rule: &Rule, tool_name: &str, args: &Value) -> bool {
        if rule.tool != "*" && rule.tool != tool_name {
            return false;
        }

        for (arg_name, matcher) in &rule.arg_patterns {
            let empty_val = Value::Null;
            let actual_val = args.get(arg_name).unwrap_or(&empty_val);
            if !matcher.matches(actual_val) {
                return false;
            }
        }
        true
    }
}

impl Matcher {
    pub fn any() -> Self {
        Self::Any
    }

    pub fn exact(val: impl Into<serde_json::Value>) -> Self {
        Self::Exact(val.into())
    }

    pub fn contains(s: impl Into<String>) -> Self {
        Self::Contains(s.into())
    }

    pub fn matches(&self, actual: &Value) -> bool {
        match self {
            Matcher::Any => true,
            Matcher::Exact(expected) => actual == expected,
            Matcher::Contains(pat) => {
                actual.as_str().map(|s| s.contains(pat)).unwrap_or(false)
            }
            Matcher::Regex(re) => {
                actual.as_str().map(|s| re.is_match(s)).unwrap_or(false)
            }
        }
    }
}
