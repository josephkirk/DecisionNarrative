//! Non-linear Narrative Node Graph model for Storylet content authoring.
//! Reference: https://github.com/shrimbly/node-banana and NarrativeFlow.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::contracts::{
    StoryletChoiceRecord, StoryletContentRecord, StoryletRecord,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodePort {
    pub id: String,
    pub name: String,
    pub port_type: String, // "input" | "output"
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NarrativeNodeData {
    Start {
        label: String,
    },
    Dialog {
        speaker_role: String,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quip: Option<String>,
    },
    Choice {
        choices: Vec<ChoiceOption>,
    },
    Variable {
        assignments: Vec<VariableAssignment>,
    },
    Conditional {
        conditions: Vec<ConditionRule>,
        #[serde(default)]
        default_route: bool,
    },
    Image {
        prompt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<String>,
    },
    Teleport {
        destination_storylet: String,
    },
    End {
        #[serde(default)]
        is_terminal: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_verb: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableAssignment {
    pub quality: String,
    pub op: String, // "set" | "add"
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConditionRule {
    pub variable: String,
    pub op: String, // "eq" | "neq" | "gt" | "gte" | "lt" | "lte"
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NarrativeNode {
    pub id: String,
    pub title: String,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub data: NarrativeNodeData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NarrativeEdge {
    pub id: String,
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NarrativeGraph {
    pub nodes: Vec<NarrativeNode>,
    pub edges: Vec<NarrativeEdge>,
}

impl NarrativeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a default narrative graph from storylet content and choices
    pub fn from_storylet(st: &StoryletRecord) -> Self {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // 1. Start node
        nodes.push(NarrativeNode {
            id: "node_start".into(),
            title: "Start".into(),
            position: (60, 120),
            size: (160, 80),
            data: NarrativeNodeData::Start { label: "Begin Scene".into() },
        });

        // 2. Dialog node with opening prose
        let prose = st.content.scene_opening.as_deref().unwrap_or(&st.content.text_template);
        nodes.push(NarrativeNode {
            id: "node_dialog_1".into(),
            title: "Dialog".into(),
            position: (280, 120),
            size: (260, 160),
            data: NarrativeNodeData::Dialog {
                speaker_role: st.content.speaker_id.clone().unwrap_or_else(|| "Narrator".into()),
                text: prose.to_string(),
                quip: None,
            },
        });

        edges.push(NarrativeEdge {
            id: "edge_start_dialog".into(),
            from_node: "node_start".into(),
            from_port: "output".into(),
            to_node: "node_dialog_1".into(),
            to_port: "input".into(),
        });

        // 3. Choice node if choices exist
        if !st.choices.is_empty() {
            let choice_options = st.choices.iter().enumerate().map(|(idx, c)| {
                ChoiceOption {
                    id: format!("choice_{idx}"),
                    text: c.label.clone(),
                    system_verb: c.system_verb.clone(),
                }
            }).collect();

            nodes.push(NarrativeNode {
                id: "node_choice_1".into(),
                title: "Choice".into(),
                position: (600, 120),
                size: (240, (st.choices.len() as i32 * 36 + 80).max(120)),
                data: NarrativeNodeData::Choice { choices: choice_options },
            });

            edges.push(NarrativeEdge {
                id: "edge_dialog_choice".into(),
                from_node: "node_dialog_1".into(),
                from_port: "output".into(),
                to_node: "node_choice_1".into(),
                to_port: "input".into(),
            });

            // For each choice, if it has a divert or end, add connected node
            for (idx, c) in st.choices.iter().enumerate() {
                if let Some(ref divert) = c.divert_target {
                    let teleport_id = format!("node_teleport_{idx}");
                    nodes.push(NarrativeNode {
                        id: teleport_id.clone(),
                        title: "Teleport".into(),
                        position: (900, (idx as i32 * 140 + 80)),
                        size: (200, 100),
                        data: NarrativeNodeData::Teleport { destination_storylet: divert.clone() },
                    });
                    edges.push(NarrativeEdge {
                        id: format!("edge_choice_{idx}_teleport"),
                        from_node: "node_choice_1".into(),
                        from_port: format!("choice_{idx}"),
                        to_node: teleport_id,
                        to_port: "input".into(),
                    });
                }
            }
        } else if st.ending {
            nodes.push(NarrativeNode {
                id: "node_end".into(),
                title: "End".into(),
                position: (600, 120),
                size: (140, 80),
                data: NarrativeNodeData::End { is_terminal: true },
            });
            edges.push(NarrativeEdge {
                id: "edge_dialog_end".into(),
                from_node: "node_dialog_1".into(),
                from_port: "output".into(),
                to_node: "node_end".into(),
                to_port: "input".into(),
            });
        }

        Self { nodes, edges }
    }

    /// Compile this node graph back into storylet prose and choices
    pub fn compile_to_storylet_content(&self, st: &mut StoryletRecord) {
        let mut prose_parts = Vec::new();
        let mut choices = Vec::new();
        let mut is_terminal = false;

        for node in &self.nodes {
            match &node.data {
                NarrativeNodeData::Dialog { text, .. } => {
                    if !text.trim().is_empty() {
                        prose_parts.push(text.trim().to_string());
                    }
                }
                NarrativeNodeData::Choice { choices: opts } => {
                    for (idx, opt) in opts.iter().enumerate() {
                        // Find connected edge from this choice port
                        let port_id = format!("choice_{idx}");
                        let target_node_id = self.edges.iter()
                            .find(|e| e.from_node == node.id && (e.from_port == port_id || e.from_port == "output"))
                            .map(|e| e.to_node.clone());

                        let divert_target = target_node_id.and_then(|tid| {
                            self.nodes.iter().find(|n| n.id == tid).and_then(|n| {
                                match &n.data {
                                    NarrativeNodeData::Teleport { destination_storylet } => Some(destination_storylet.clone()),
                                    NarrativeNodeData::End { .. } => Some("ending".into()),
                                    _ => None,
                                }
                            })
                        });

                        choices.push(StoryletChoiceRecord {
                            choice_id: format!("choice_{}", idx + 1),
                            label: opt.text.clone(),
                            system_verb: opt.system_verb.clone().or_else(|| Some("ACT".into())),
                            conditions: None,
                            effects: json!({}),
                            divert_target,
                            advisor_preview: None,
                        });
                    }
                }
                NarrativeNodeData::End { is_terminal: term } => {
                    if *term {
                        is_terminal = true;
                    }
                }
                _ => {}
            }
        }

        if !prose_parts.is_empty() {
            let full_prose = prose_parts.join("\n\n");
            st.content.scene_opening = Some(full_prose.clone());
            st.content.text_template = full_prose;
        }

        if !choices.is_empty() {
            st.choices = choices;
        }

        if is_terminal {
            st.ending = true;
        }
    }
}
