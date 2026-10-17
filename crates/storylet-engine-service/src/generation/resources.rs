//! Story craft resource registry and SHA-256 provenance calculator.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoryCraftResourceReference {
    pub id: String,
    pub version: String,
    pub source: String,
    #[serde(rename = "sourceHash")]
    pub source_hash: String,
    #[serde(rename = "promptHash")]
    pub prompt_hash: String,
}

#[derive(Clone, Debug)]
pub struct LoadedStoryCraftResources {
    pub prompt: String,
    pub references: Vec<StoryCraftResourceReference>,
}

struct ResourceDef {
    id: &'static str,
    source: &'static str,
    guidance: &'static str,
}

const RESOURCES: &[ResourceDef] = &[
    ResourceDef {
        id: "character-management",
        source: ".agents/skills/character-management/SKILL.md",
        guidance: "For each role, preserve the designer's supplied goals, boundaries, relationships, agency, and knowledge. Give each character a motive and tactic; do not invent profile facts.",
    },
    ResourceDef {
        id: "worldbuilding",
        source: ".agents/skills/worldbuilding/SKILL.md",
        guidance: "Treat setting facts as authored canon. Ground action in available place, objects, social systems, and constraints. Mark unknown details instead of filling gaps as fact.",
    },
    ResourceDef {
        id: "scene-craft",
        source: ".agents/skills/scene-craft/SKILL.md",
        guidance: "Work at scene grain: concrete goal, resistance, tactic, subtext, beat progression, and an outcome that changes the next playable situation. Keep physical details causally relevant.",
    },
    ResourceDef {
        id: "interactive-fiction",
        source: ".agents/skills/interactive-fiction/SKILL.md",
        guidance: "Make each choice understandable before selection and distinct in intent and consequence. Preserve branch continuity and ensure branch consequences follow the declared storylet effects.",
    },
    ResourceDef {
        id: "voice-style",
        source: ".agents/skills/voice-style/SKILL.md",
        guidance: "Keep dialogue and narration consistent with project-owned voice constraints. Differentiate speakers through word choice, rhythm, attention, and tactics; do not infer a house style when none is supplied.",
    },
    ResourceDef {
        id: "line-editing",
        source: ".agents/skills/line-editing/SKILL.md",
        guidance: "Review prose for clarity, specificity, rhythm, point of view, and redundant explanation. Report edits and reasons; do not silently alter authored text or executable effects.",
    },
    ResourceDef {
        id: "revision-continuity",
        source: ".agents/skills/revision-continuity/SKILL.md",
        guidance: "Check event order, character knowledge, relationships, location facts, and prior branch consequences against supplied canon and earlier stages. Distinguish contradictions from unresolved unknowns.",
    },
];

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub struct CraftRegistry {
    root: PathBuf,
}

impl CraftRegistry {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn reference(&self, id: &str) -> Result<StoryCraftResourceReference, String> {
        let def = RESOURCES
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| format!("Unknown craft resource '{id}'"))?;

        let abs_path = self.root.join(def.source);
        let raw = std::fs::read_to_string(&abs_path)
            .map_err(|e| format!("Craft resource source '{}' unavailable: {e}", def.source))?;
        let normalized = raw.replace(
            "
", "
",
        );

        Ok(StoryCraftResourceReference {
            id: def.id.to_string(),
            version: "1.0.0".to_string(),
            source: def.source.replace('\\', "/"),
            source_hash: sha256_hex(&normalized),
            prompt_hash: sha256_hex(def.guidance),
        })
    }

    pub fn catalog(&self) -> Result<Vec<StoryCraftResourceReference>, String> {
        RESOURCES.iter().map(|r| self.reference(r.id)).collect()
    }

    pub fn for_stage(&self, stage: &str) -> Result<LoadedStoryCraftResources, String> {
        let selected_ids: &[&str] = match stage {
            "canon_and_voice" => &[
                "character-management",
                "worldbuilding",
                "voice-style",
                "revision-continuity",
            ],
            "physical_seed" => &["worldbuilding", "scene-craft"],
            "premise_options" => &["scene-craft", "interactive-fiction"],
            "scene_plan" => &[
                "character-management",
                "worldbuilding",
                "scene-craft",
                "voice-style",
            ],
            "storylet_contract" => &["interactive-fiction", "revision-continuity"],
            "scene_variants" => &["scene-craft", "interactive-fiction", "voice-style"],
            "craft_audit" => &[
                "character-management",
                "worldbuilding",
                "scene-craft",
                "interactive-fiction",
                "voice-style",
                "line-editing",
                "revision-continuity",
            ],
            "revision" => &[
                "scene-craft",
                "interactive-fiction",
                "voice-style",
                "line-editing",
                "revision-continuity",
            ],
            "review_package" => &[
                "interactive-fiction",
                "voice-style",
                "line-editing",
                "revision-continuity",
            ],
            _ => &[],
        };

        let mut references = Vec::new();
        let mut prompt_sections = Vec::new();

        for &id in selected_ids {
            let ref_item = self.reference(id)?;
            let def = RESOURCES.iter().find(|r| r.id == id).unwrap();
            prompt_sections.push(format!(
                "CRAFT RESOURCE {}/1.0.0:
{}",
                id, def.guidance
            ));
            references.push(ref_item);
        }

        Ok(LoadedStoryCraftResources {
            prompt: prompt_sections.join(
                "

",
            ),
            references,
        })
    }
}
