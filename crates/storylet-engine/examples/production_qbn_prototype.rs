//! Prototype: all-bindings production matching for QBN / salience decks.
//!
//! `WorldStateTable::match_graph_pattern` returns only the FIRST match of a
//! pattern inside `PATTERN_SEARCH_BUDGET`, which is enough for graph
//! transformations but not enough for QBN: a storylet deck needs every
//! binding, because each binding is a distinct option ("trade the poison with
//! the wizard", "trade the poison with the guard").
//!
//! This example enumerates all bindings over the real `WorldStateTable` types
//! and measures the cost, so the design document can state numbers instead of
//! guesses.
//!
//! Run: `cargo run --release --manifest-path crates/storylet-engine/Cargo.toml --example production_qbn_prototype`

use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::time::Instant;

use storylet_engine::chrono_state::{
    EdgePattern, GraphBinding, GraphLayer, GraphPattern, VertexPattern, WorldEdge, WorldStateTable,
    WorldVertex,
};

/// A production (paper §3.2): a left-hand side pattern that, when it matches,
/// contributes a storylet to the deck.
struct ProductionDef {
    key: &'static str,
    lhs: GraphPattern,
    /// Production this one specializes. When the child matches, the parent is
    /// suppressed for that context (paper §4.4 generic vs. detailed).
    generic_parent: Option<&'static str>,
    specificity: i32,
    storylet: &'static str,
}

/// One concrete binding of a production's left-hand side to world vertices and
/// edges. `Ord` so bindings can be deduplicated and tie-broken deterministically.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Binding {
    vertices: Vec<String>,
    edges: Vec<String>,
}

#[derive(Clone)]
struct MatchRun {
    bindings: Vec<Binding>,
    vertex_assignments: u64,
    edge_checks: u64,
    truncated: bool,
}

/// One row of the deck: a storylet available under one binding.
struct OptionRow {
    storylet: &'static str,
    production: &'static str,
    binding: Binding,
    score: f64,
}

#[derive(Default)]
struct Resolution {
    options: Vec<OptionRow>,
    matches: BTreeMap<&'static str, usize>,
    suppressed_generic: BTreeSet<&'static str>,
}

fn id(prefix: &str, index: usize) -> String {
    format!("{prefix}.{index:05}")
}

// ---------------------------------------------------------------- world build

fn build_world(locations: usize, characters: usize, items: usize) -> WorldStateTable {
    let mut graph = WorldStateTable::new();

    for i in 0..locations {
        graph.vertices.push(WorldVertex {
            vertex_id: id("loc", i),
            layer: GraphLayer::Location,
            entity_index: None,
            attributes: BTreeMap::new(),
        });
    }
    for i in 0..characters {
        graph.vertices.push(WorldVertex {
            vertex_id: id("char", i),
            layer: GraphLayer::Character,
            entity_index: None,
            attributes: BTreeMap::new(),
        });
    }
    for i in 0..items {
        graph.vertices.push(WorldVertex {
            vertex_id: id("item", i),
            layer: GraphLayer::Item,
            entity_index: None,
            attributes: BTreeMap::new(),
        });
    }
    // Vertex lookup uses `partition_point`, so the table must stay sorted.
    graph.vertices.sort_by(|a, b| a.vertex_id.cmp(&b.vertex_id));

    for i in 0..characters {
        graph.edges.push(WorldEdge {
            edge_id: id("edge.owns", i),
            source_id: id("char", i),
            target_id: id("item", i % items.max(1)),
            relation_type: "owns".to_string(),
            weight: 1.0,
        });
    }
    for i in 0..characters {
        graph.edges.push(WorldEdge {
            edge_id: id("edge.located", i),
            source_id: id("char", i),
            target_id: id("loc", i % locations.max(1)),
            relation_type: "located_at".to_string(),
            weight: 1.0,
        });
    }
    for i in 0..locations.saturating_sub(1) {
        graph.edges.push(WorldEdge {
            edge_id: id("edge.connected", i),
            source_id: id("loc", i),
            target_id: id("loc", i + 1),
            relation_type: "connected".to_string(),
            weight: 1.0,
        });
    }
    graph.edges.sort_by(|a, b| a.edge_id.cmp(&b.edge_id));
    graph
}

// ------------------------------------------------------------ pattern builders

fn vp(layer: GraphLayer) -> VertexPattern {
    VertexPattern {
        layer,
        vertex_id: None,
        attributes: Vec::new(),
    }
}

fn vp_literal(layer: GraphLayer, vertex_id: &str) -> VertexPattern {
    VertexPattern {
        layer,
        vertex_id: Some(vertex_id.to_string()),
        attributes: Vec::new(),
    }
}

fn edge(relation: &str, source: usize, target: usize) -> EdgePattern {
    EdgePattern {
        edge_id: None,
        relation_type: Some(relation.to_string()),
        source: Some(GraphBinding::VertexPattern(source)),
        target: Some(GraphBinding::VertexPattern(target)),
    }
}

/// `?character owns ?item`
fn pattern_char_item_owns() -> GraphPattern {
    GraphPattern {
        vertices: vec![vp(GraphLayer::Character), vp(GraphLayer::Item)],
        edges: vec![edge("owns", 0, 1)],
    }
}

/// `char.A owns item.B` for literal ids.
fn pattern_specific_owns(character: &str, item: &str) -> GraphPattern {
    GraphPattern {
        vertices: vec![
            vp_literal(GraphLayer::Character, character),
            vp_literal(GraphLayer::Item, item),
        ],
        edges: vec![edge("owns", 0, 1)],
    }
}

/// `?character owns ?item` and `?character located_at ?location`
fn pattern_char_item_location() -> GraphPattern {
    GraphPattern {
        vertices: vec![
            vp(GraphLayer::Character),
            vp(GraphLayer::Item),
            vp(GraphLayer::Location),
        ],
        edges: vec![edge("owns", 0, 1), edge("located_at", 0, 2)],
    }
}

// ------------------------------------------------------- all-bindings matching

struct Matcher<'a> {
    graph: &'a WorldStateTable,
    pattern: &'a GraphPattern,
    cap: usize,
    cands: Vec<Vec<String>>,
    used_vertices: BTreeSet<String>,
    used_edges: BTreeSet<String>,
    chosen: Vec<String>,
    matched_edges: Vec<String>,
    vertex_assignments: u64,
    edge_checks: u64,
    bindings: Vec<Binding>,
    truncated: bool,
}

impl<'a> Matcher<'a> {
    fn edge_matches(&self, pattern_index: usize, edge_index: usize) -> bool {
        let ep = &self.pattern.edges[pattern_index];
        let edge = &self.graph.edges[edge_index];
        if let Some(wanted) = &ep.edge_id {
            if &edge.edge_id != wanted {
                return false;
            }
        }
        if let Some(wanted) = &ep.relation_type {
            if &edge.relation_type != wanted {
                return false;
            }
        }
        if let Some(binding) = &ep.source {
            match binding {
                GraphBinding::Literal(wanted) => {
                    if &edge.source_id != wanted {
                        return false;
                    }
                }
                GraphBinding::VertexPattern(index) => {
                    if self.chosen[*index] != edge.source_id {
                        return false;
                    }
                }
                // An edge-pattern endpoint cannot match a vertex endpoint here.
                GraphBinding::EdgePattern(_) => return false,
            }
        }
        if let Some(binding) = &ep.target {
            match binding {
                GraphBinding::Literal(wanted) => {
                    if &edge.target_id != wanted {
                        return false;
                    }
                }
                GraphBinding::VertexPattern(index) => {
                    if self.chosen[*index] != edge.target_id {
                        return false;
                    }
                }
                GraphBinding::EdgePattern(_) => return false,
            }
        }
        true
    }

    /// Returns `true` when the caller must unwind (cap reached).
    fn match_vertices(&mut self, index: usize) -> bool {
        if index == self.pattern.vertices.len() {
            self.vertex_assignments += 1;
            return self.match_edges(0);
        }
        let count = self.cands[index].len();
        for k in 0..count {
            let vertex_id = self.cands[index][k].clone();
            if self.used_vertices.contains(&vertex_id) {
                continue;
            }
            self.used_vertices.insert(vertex_id.clone());
            self.chosen.push(vertex_id.clone());
            if self.match_vertices(index + 1) {
                return true;
            }
            self.chosen.pop();
            self.used_vertices.remove(&vertex_id);
        }
        false
    }

    /// Returns `true` when the caller must unwind (cap reached).
    fn match_edges(&mut self, index: usize) -> bool {
        if index == self.pattern.edges.len() {
            self.bindings.push(Binding {
                vertices: self.chosen.clone(),
                edges: self.matched_edges.clone(),
            });
            if self.bindings.len() >= self.cap {
                self.truncated = true;
                return true;
            }
            return false;
        }
        let edge_count = self.graph.edges.len();
        for edge_index in 0..edge_count {
            self.edge_checks += 1;
            if !self.edge_matches(index, edge_index) {
                continue;
            }
            let edge_id = self.graph.edges[edge_index].edge_id.clone();
            if self.used_edges.contains(&edge_id) {
                continue;
            }
            self.used_edges.insert(edge_id.clone());
            self.matched_edges.push(edge_id.clone());
            if self.match_edges(index + 1) {
                return true;
            }
            self.matched_edges.pop();
            self.used_edges.remove(&edge_id);
        }
        false
    }
}

fn match_all(graph: &WorldStateTable, pattern: &GraphPattern, cap: usize) -> MatchRun {
    let mut matcher = Matcher {
        graph,
        pattern,
        cap,
        cands: (0..pattern.vertices.len())
            .map(|index| {
                let vp = &pattern.vertices[index];
                graph
                    .vertices
                    .iter()
                    .filter(|vertex| {
                        vertex.layer == vp.layer
                            && vp
                                .vertex_id
                                .as_ref()
                                .is_none_or(|wanted| &vertex.vertex_id == wanted)
                    })
                    .map(|vertex| vertex.vertex_id.clone())
                    .collect()
            })
            .collect(),
        used_vertices: BTreeSet::new(),
        used_edges: BTreeSet::new(),
        chosen: Vec::with_capacity(pattern.vertices.len()),
        matched_edges: Vec::with_capacity(pattern.edges.len()),
        vertex_assignments: 0,
        edge_checks: 0,
        bindings: Vec::new(),
        truncated: false,
    };
    matcher.match_vertices(0);
    let mut bindings = matcher.bindings;
    bindings.sort();
    bindings.dedup();
    MatchRun {
        bindings,
        vertex_assignments: matcher.vertex_assignments,
        edge_checks: matcher.edge_checks,
        truncated: matcher.truncated,
    }
}

// --------------------------------------------------------------- deck resolve

fn resolve(productions: &[ProductionDef], graph: &WorldStateTable) -> Resolution {
    let mut resolution = Resolution::default();
    let mut matched_per_production: Vec<(&'static str, MatchRun)> = Vec::new();
    for production in productions {
        let run = match_all(graph, &production.lhs, 250_000);
        resolution
            .matches
            .insert(production.key, run.bindings.len());
        matched_per_production.push((production.key, run));
    }
    for production in productions {
        let Some(parent) = production.generic_parent else {
            continue;
        };
        let count = resolution.matches.get(production.key).copied().unwrap_or(0);
        if count > 0 {
            resolution.suppressed_generic.insert(parent);
        }
    }
    for production in productions {
        if resolution.suppressed_generic.contains(production.key) {
            continue;
        }
        let run = matched_per_production
            .iter()
            .find(|(key, _)| *key == production.key)
            .map(|(_, run)| run);
        let Some(run) = run else { continue };
        for binding in &run.bindings {
            resolution.options.push(OptionRow {
                storylet: production.storylet,
                production: production.key,
                binding: binding.clone(),
                score: f64::from(production.specificity),
            });
        }
    }
    resolution
}

// ------------------------------------------------------------------ measurement

fn measure(
    label: &str,
    graph: &WorldStateTable,
    pattern: &GraphPattern,
    iterations: usize,
) -> MatchRun {
    let warm = match_all(graph, pattern, 250_000);
    let start = Instant::now();
    let mut last = warm.clone();
    for _ in 0..iterations {
        last = black_box(match_all(black_box(graph), black_box(pattern), 250_000));
    }
    let elapsed = start.elapsed();
    let average_ms = elapsed.as_secs_f64() * 1000.0 / iterations as f64;
    println!(
        "{label}: bindings={} vertex_assignments={} edge_checks={} avg_ms={average_ms:.3} truncated={}",
        last.bindings.len(),
        last.vertex_assignments,
        last.edge_checks,
        last.truncated
    );
    last
}

fn main() {
    // ---------------------------------------------------------- 1. precedence
    let world = build_world(40, 160, 100);
    println!(
        "vertices={} edges={}",
        world.vertices.len(),
        world.edges.len()
    );

    let productions = vec![
        ProductionDef {
            key: "trade.any",
            lhs: pattern_char_item_owns(),
            generic_parent: None,
            specificity: 1,
            storylet: "trade.basic",
        },
        ProductionDef {
            key: "trade.poison_from_wizard",
            lhs: pattern_specific_owns("char.00042", "item.00042"),
            generic_parent: Some("trade.any"),
            specificity: 10,
            storylet: "trade.poison",
        },
        ProductionDef {
            key: "trade.missing",
            lhs: pattern_specific_owns("char.99999", "item.99999"),
            generic_parent: Some("trade.any"),
            specificity: 20,
            storylet: "trade.unreachable",
        },
    ];
    let resolution = resolve(&productions, &world);
    println!(
        "precedence: generic_matches={} detailed_matches={} missing_matches={} suppressed={:?} options={}",
        resolution.matches["trade.any"],
        resolution.matches["trade.poison_from_wizard"],
        resolution.matches["trade.missing"],
        resolution.suppressed_generic,
        resolution.options.len()
    );
    assert_eq!(resolution.matches["trade.any"], 160);
    assert_eq!(resolution.matches["trade.poison_from_wizard"], 1);
    assert_eq!(resolution.matches["trade.missing"], 0);
    assert!(resolution.suppressed_generic.contains("trade.any"));
    assert_eq!(resolution.options.len(), 1);
    assert_eq!(resolution.options[0].production, "trade.poison_from_wizard");
    assert_eq!(
        resolution.options[0].binding.vertices,
        vec!["char.00042".to_string(), "item.00042".to_string()]
    );

    // ------------------------------------------------------- 2. generic fallback
    let fallback = vec![
        ProductionDef {
            key: "talk.any",
            lhs: GraphPattern {
                vertices: vec![vp(GraphLayer::Character)],
                edges: Vec::new(),
            },
            generic_parent: None,
            specificity: 1,
            storylet: "talk.basic",
        },
        ProductionDef {
            key: "talk.missing",
            lhs: GraphPattern {
                vertices: vec![vp_literal(GraphLayer::Character, "char.99999")],
                edges: Vec::new(),
            },
            generic_parent: Some("talk.any"),
            specificity: 5,
            storylet: "talk.unreachable",
        },
    ];
    let fallback_resolution = resolve(&fallback, &world);
    println!(
        "fallback: any_matches={} missing_matches={} suppressed={:?} options={}",
        fallback_resolution.matches["talk.any"],
        fallback_resolution.matches["talk.missing"],
        fallback_resolution.suppressed_generic,
        fallback_resolution.options.len()
    );
    assert!(fallback_resolution.suppressed_generic.is_empty());
    assert_eq!(fallback_resolution.options.len(), 160);

    // ---------------------------------------------------------- 3. deck growth
    let single = vec![ProductionDef {
        key: "talk.single",
        lhs: GraphPattern {
            vertices: vec![vp(GraphLayer::Character)],
            edges: Vec::new(),
        },
        generic_parent: None,
        specificity: 1,
        storylet: "talk.single",
    }];
    let generic_options = resolve(&single, &world).options.len();

    let doubled = vec![
        ProductionDef {
            key: "talk.opener",
            lhs: GraphPattern {
                vertices: vec![vp(GraphLayer::Character)],
                edges: Vec::new(),
            },
            generic_parent: None,
            specificity: 1,
            storylet: "talk.opener",
        },
        ProductionDef {
            key: "talk.rumor",
            lhs: GraphPattern {
                vertices: vec![vp(GraphLayer::Character)],
                edges: Vec::new(),
            },
            generic_parent: None,
            specificity: 1,
            storylet: "talk.rumor",
        },
    ];
    let doubled_options = resolve(&doubled, &world).options.len();
    let bindings_per_storylet = generic_options;
    println!(
        "deck: bindings_per_storylet={bindings_per_storylet} generic_options={generic_options} two_storylet_options={doubled_options}"
    );
    assert_eq!(generic_options, 160);
    assert_eq!(doubled_options, 320);

    // -------------------------------------------------------------- 4. salience
    let salient_deck = vec![ProductionDef {
        key: "talk.any",
        lhs: GraphPattern {
            vertices: vec![vp(GraphLayer::Character)],
            edges: Vec::new(),
        },
        generic_parent: None,
        specificity: 2,
        storylet: "talk.any",
    }];
    let salient = resolve(&salient_deck, &world);
    assert_eq!(salient.options.len(), 160);
    let picked = salient
        .options
        .iter()
        .max_by(|left, right| {
            left.score
                .total_cmp(&right.score)
                .then_with(|| right.binding.cmp(&left.binding))
        })
        .expect("deck is non-empty");
    println!(
        "salience: options={} selected_storylet={} selected_production={} selected_binding={:?}",
        salient.options.len(),
        picked.storylet,
        picked.production,
        picked.binding.vertices
    );
    // Pin the tie-break itself: reversing the comparator still yields 160
    // options, so the deck size alone proves nothing.
    assert_eq!(picked.storylet, "talk.any");
    assert_eq!(picked.production, "talk.any");
    assert_eq!(picked.score, 2.0);
    assert_eq!(picked.binding.vertices, vec!["char.00000".to_string()]);
    assert!(picked.binding.edges.is_empty());

    // ------------------------------------------------------------------ 5. cost
    measure(
        "match_all char+item owns",
        &world,
        &pattern_char_item_owns(),
        20,
    );
    let compound = measure(
        "match_all char+item+location",
        &world,
        &pattern_char_item_location(),
        3,
    );
    assert_eq!(compound.bindings.len(), 160);
    assert!(!compound.truncated);

    // -------------------------------------------------------------- 6. baseline
    let pattern = pattern_char_item_owns();
    let iterations = 20;
    let _warm = world.match_graph_pattern(&pattern).expect("warm-up match");
    let start = Instant::now();
    let mut success = 0;
    for _ in 0..iterations {
        if world.match_graph_pattern(black_box(&pattern)).is_ok() {
            success += 1;
        }
    }
    let elapsed = start.elapsed();
    println!(
        "first-only baseline: successful_runs={success} avg_ms={:.4}",
        elapsed.as_secs_f64() * 1000.0 / iterations as f64
    );
    assert_eq!(success, iterations);
}
