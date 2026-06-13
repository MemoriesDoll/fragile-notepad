use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::platform::paths;

use self::cache_xml::{registry_from_cache_xml, registry_to_cache_xml};
use super::compiler::{CompiledOutlineRegistry, OutlinePlan, compile_outline_schema};
use super::diagnostics;
use super::schema::parse_outline_schema;
use super::types::OutlineDiagnostic;

const CACHE_VERSION: &str = "1";
const CACHE_FILE: &str = "outline-registry-v1.xml";
const CACHE_PATH_ENV: &str = "FRAGILE_NOTEPAD_OUTLINE_CACHE_PATH";

mod cache_xml;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineRegistry {
    hash: u64,
    plans: Vec<OutlinePlan>,
    token_index: HashMap<String, usize>,
    diagnostics: Vec<OutlineDiagnostic>,
}

impl OutlineRegistry {
    pub fn load() -> Self {
        Self::shared().clone()
    }

    pub fn shared() -> &'static Self {
        static REGISTRY: OnceLock<OutlineRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::load_uncached)
    }

    fn load_uncached() -> Self {
        let xml = crate::assets::syntax::outline_parsers_xml();
        let hash = deterministic_hash(xml);

        if let Some(registry) = load_compiled_cache(hash) {
            return registry;
        }

        let (schema, parse_diagnostics) = parse_outline_schema(xml);
        let registry = Self::from_compiled(hash, compile_outline_schema(schema, parse_diagnostics));
        let _ = save_compiled_cache(&registry);

        registry
    }

    pub fn from_xml(xml: &str) -> Self {
        let hash = deterministic_hash(xml);
        let (schema, parse_diagnostics) = parse_outline_schema(xml);
        Self::from_compiled(hash, compile_outline_schema(schema, parse_diagnostics))
    }

    pub fn registry_hash(&self) -> u64 {
        self.hash
    }

    pub fn plan_for_syntax(&self, syntax_token: &str) -> Option<&OutlinePlan> {
        let syntax_token = syntax_token.trim().to_ascii_lowercase();
        self.token_index
            .get(&syntax_token)
            .and_then(|index| self.plans.get(*index))
    }

    pub fn plans(&self) -> &[OutlinePlan] {
        &self.plans
    }

    pub fn diagnostics(&self) -> &[OutlineDiagnostic] {
        &self.diagnostics
    }

    fn from_compiled(hash: u64, compiled: CompiledOutlineRegistry) -> Self {
        let CompiledOutlineRegistry {
            plans,
            mut diagnostics,
        } = compiled;
        let mut token_index = HashMap::new();

        for (plan_index, plan) in plans.iter().enumerate() {
            for token in &plan.syntax_tokens {
                if token_index.insert(token.clone(), plan_index).is_some() {
                    diagnostics.push(diagnostics::warning(format!(
                        "outline parser token {token} is defined by multiple language plans; last definition wins"
                    )));
                }
            }
        }

        Self {
            hash,
            plans,
            token_index,
            diagnostics,
        }
    }
}

fn load_compiled_cache(expected_hash: u64) -> Option<OutlineRegistry> {
    let path = cache_path()?;
    load_compiled_cache_from_path(expected_hash, &path)
}

fn save_compiled_cache(registry: &OutlineRegistry) -> Option<()> {
    let path = cache_path()?;
    save_compiled_cache_to_path(registry, &path)
}

fn load_compiled_cache_from_path(expected_hash: u64, path: &Path) -> Option<OutlineRegistry> {
    let contents = fs::read_to_string(path).ok()?;
    registry_from_cache_xml(&contents).filter(|registry| registry.registry_hash() == expected_hash)
}

fn save_compiled_cache_to_path(registry: &OutlineRegistry, path: &Path) -> Option<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok()?;
    }

    fs::write(path, registry_to_cache_xml(registry)).ok()
}

fn cache_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os(CACHE_PATH_ENV) {
        return Some(PathBuf::from(path));
    }

    if cfg!(test) {
        return None;
    }

    cache_dir().map(|path| path.join(CACHE_FILE))
}

fn cache_dir() -> Option<PathBuf> {
    paths::cache_dir()
}

fn deterministic_hash(input: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    input.as_bytes().iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::outline::compiler::OutlineBodyKind;
    use crate::editor::outline::types::OutlineNodeKind;

    #[test]
    fn included_registry_resolves_required_language_tokens() {
        let registry = OutlineRegistry::load();

        assert!(registry.diagnostics().is_empty());
        assert!(registry.registry_hash() != 0);
        assert_eq!(
            registry
                .plan_for_syntax("rs")
                .map(|plan| plan.adapter_name.as_str()),
            Some("rust")
        );
        assert_eq!(
            registry
                .plan_for_syntax("PY")
                .map(|plan| plan.adapter_name.as_str()),
            Some("python")
        );
        assert_eq!(
            registry
                .plan_for_syntax("js")
                .map(|plan| plan.adapter_name.as_str()),
            Some("javascript")
        );
        assert_eq!(
            registry
                .plan_for_syntax("java")
                .map(|plan| plan.adapter_name.as_str()),
            Some("generic-brace")
        );
        assert_eq!(
            registry
                .plan_for_syntax("kt")
                .map(|plan| plan.adapter_name.as_str()),
            Some("generic-brace")
        );
        assert_eq!(
            registry
                .plan_for_syntax("cpp")
                .map(|plan| plan.adapter_name.as_str()),
            Some("generic-brace")
        );
        assert_eq!(
            registry
                .plan_for_syntax("HPP")
                .map(|plan| plan.adapter_name.as_str()),
            Some("generic-brace")
        );
        assert_eq!(
            registry
                .plan_for_syntax("rb")
                .map(|plan| plan.adapter_name.as_str()),
            Some("ruby")
        );
    }

    #[test]
    fn registry_exposes_compiled_plan_metadata_for_document_parser() {
        let registry = OutlineRegistry::load();
        let ruby = registry.plan_for_syntax("rb").unwrap();

        assert_eq!(ruby.family_id, "end-keyword");
        assert_eq!(ruby.structure.bodies[0].kind, OutlineBodyKind::EndKeyword);
        assert!(
            ruby.containers
                .iter()
                .any(|rule| rule.node_kind == OutlineNodeKind::Class)
        );
        assert!(
            ruby.declarations
                .iter()
                .any(|rule| rule.node_kind == OutlineNodeKind::Function
                    && rule.method_containers.contains(&OutlineNodeKind::Class)
                    && rule.method_containers.contains(&OutlineNodeKind::Module))
        );
    }

    #[test]
    fn malformed_registry_falls_back_to_empty_registry_with_diagnostics() {
        let registry = OutlineRegistry::from_xml("<outline-parsers>");

        assert!(registry.plans().is_empty());
        assert!(registry.plan_for_syntax("rs").is_none());
        assert!(!registry.diagnostics().is_empty());
    }

    #[test]
    fn registry_hash_is_deterministic() {
        assert_eq!(deterministic_hash("abc"), deterministic_hash("abc"));
        assert_ne!(deterministic_hash("abc"), deterministic_hash("abd"));
    }

    #[test]
    fn shared_registry_reuses_compiled_instance() {
        let first = OutlineRegistry::shared();
        let second = OutlineRegistry::shared();

        assert!(std::ptr::eq(first, second));
        assert_eq!(OutlineRegistry::load(), first.clone());
    }

    #[test]
    fn compiled_cache_round_trips_first_party_registry() {
        let registry = OutlineRegistry::load();
        let xml = registry_to_cache_xml(&registry);
        let cached = registry_from_cache_xml(&xml).expect("compiled cache should parse");

        assert_eq!(cached, registry);
        assert_eq!(
            cached
                .plan_for_syntax("rs")
                .map(|plan| plan.adapter_name.as_str()),
            Some("rust")
        );
        assert_eq!(
            cached
                .plan_for_syntax("cpp")
                .map(|plan| plan.declarations[0].callable.operator_tokens.as_slice()),
            registry
                .plan_for_syntax("cpp")
                .map(|plan| plan.declarations[0].callable.operator_tokens.as_slice())
        );
    }

    #[test]
    fn compiled_cache_rejects_stale_hash() {
        let registry = OutlineRegistry::load();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "fragile-notepad-outline-cache-stale-{}.xml",
            std::process::id()
        ));

        save_compiled_cache_to_path(&registry, &path).expect("cache should write");

        assert!(load_compiled_cache_from_path(registry.registry_hash() + 1, &path).is_none());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn compiled_cache_rejects_corrupt_xml() {
        assert!(registry_from_cache_xml("<compiled-outline-cache>").is_none());
    }
}
