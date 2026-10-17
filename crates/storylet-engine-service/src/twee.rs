//! Twee 3 and Twine 2 HTML parser, serializer, and storylet converter.
//! Strictly conforms to iftechfoundation/twine-specs.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::contracts::{StoryletChoiceRecord, StoryletContentRecord, StoryletRecord};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwineLink {
    pub display: String,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TweePassage {
    pub name: String,
    pub tags: Vec<String>,
    pub position: Option<(i32, i32)>,
    pub size: Option<(i32, i32)>,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TweeStory {
    pub title: String,
    pub ifid: String,
    pub format: String,
    pub format_version: String,
    pub start_passage: Option<String>,
    pub zoom: f64,
    pub passages: Vec<TweePassage>,
}

impl Default for TweeStory {
    fn default() -> Self {
        Self {
            title: "Untitled Twine Story".into(),
            ifid: uuid::Uuid::new_v4().to_string().to_uppercase(),
            format: "SugarCube".into(),
            format_version: "2.36.1".into(),
            start_passage: None,
            zoom: 1.0,
            passages: Vec::new(),
        }
    }
}

pub fn extract_twine_links(text: &str) -> Vec<TwineLink> {
    let mut links = Vec::new();
    let re = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();

    for cap in re.captures_iter(text) {
        let inner = cap[1].trim();
        if inner.is_empty() {
            continue;
        }

        if let Some((disp, tgt)) = inner.split_once("->") {
            links.push(TwineLink {
                display: disp.trim().to_string(),
                target: tgt.trim().to_string(),
            });
        } else if let Some((tgt, disp)) = inner.split_once("<-") {
            links.push(TwineLink {
                display: disp.trim().to_string(),
                target: tgt.trim().to_string(),
            });
        } else if let Some((disp, tgt)) = inner.split_once('|') {
            links.push(TwineLink {
                display: disp.trim().to_string(),
                target: tgt.trim().to_string(),
            });
        } else {
            links.push(TwineLink {
                display: inner.to_string(),
                target: inner.to_string(),
            });
        }
    }

    links
}

fn parse_passage_header(
    line: &str,
) -> Option<(String, Vec<String>, Option<(i32, i32)>, Option<(i32, i32)>)> {
    let line = line.trim();
    if !line.starts_with("::") {
        return None;
    }
    let rest = line.trim_start_matches(':').trim();
    if rest.is_empty() {
        return None;
    }

    let mut tags = Vec::new();
    let mut pos = None;
    let mut size = None;

    let mut name = rest;

    // Check for trailing metadata { ... }
    if let Some(start_brace) = rest.rfind('{') {
        if let Some(end_brace) = rest.rfind('}') {
            if end_brace > start_brace {
                let meta_str = &rest[start_brace..=end_brace];
                if let Ok(meta_json) = serde_json::from_str::<Value>(meta_str) {
                    if let Some(pos_str) = meta_json.get("position").and_then(|v| v.as_str()) {
                        if let Some((x_s, y_s)) = pos_str.split_once(',') {
                            if let (Ok(x), Ok(y)) =
                                (x_s.trim().parse::<i32>(), y_s.trim().parse::<i32>())
                            {
                                pos = Some((x, y));
                            }
                        }
                    }
                    if let Some(size_str) = meta_json.get("size").and_then(|v| v.as_str()) {
                        if let Some((w_s, h_s)) = size_str.split_once(',') {
                            if let (Ok(w), Ok(h)) =
                                (w_s.trim().parse::<i32>(), h_s.trim().parse::<i32>())
                            {
                                size = Some((w, h));
                            }
                        }
                    }
                }
                name = rest[..start_brace].trim();
            }
        }
    }

    // Check for trailing tags [ ... ]
    if let Some(start_bracket) = name.rfind('[') {
        if let Some(end_bracket) = name.rfind(']') {
            if end_bracket > start_bracket {
                let tags_str = &name[start_bracket + 1..end_bracket];
                tags = tags_str.split_whitespace().map(|s| s.to_string()).collect();
                name = name[..start_bracket].trim();
            }
        }
    }

    Some((name.to_string(), tags, pos, size))
}

pub fn parse_twee_3(input: &str) -> Result<TweeStory, String> {
    let mut story = TweeStory::default();
    let mut current_header: Option<(String, Vec<String>, Option<(i32, i32)>, Option<(i32, i32)>)> =
        None;
    let mut current_body_lines: Vec<&str> = Vec::new();

    for line in input.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("::") {
            if let Some((name, tags, pos, size)) = current_header.take() {
                let body = current_body_lines.join("\n").trim().to_string();
                process_passage(&mut story, name, tags, pos, size, body)?;
                current_body_lines.clear();
            }

            if let Some(header) = parse_passage_header(trimmed_start) {
                current_header = Some(header);
            }
        } else if current_header.is_some() {
            current_body_lines.push(line);
        }
    }

    if let Some((name, tags, pos, size)) = current_header.take() {
        let body = current_body_lines.join("\n").trim().to_string();
        process_passage(&mut story, name, tags, pos, size, body)?;
    }

    Ok(story)
}
fn process_passage(
    story: &mut TweeStory,
    name: String,
    tags: Vec<String>,
    pos: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
    body: String,
) -> Result<(), String> {
    if name == "StoryTitle" {
        story.title = body.lines().next().unwrap_or("Untitled").trim().to_string();
    } else if name == "StoryData" {
        if let Ok(data) = serde_json::from_str::<Value>(&body) {
            if let Some(ifid) = data.get("ifid").and_then(|v| v.as_str()) {
                story.ifid = ifid.to_string();
            }
            if let Some(fmt) = data.get("format").and_then(|v| v.as_str()) {
                story.format = fmt.to_string();
            }
            if let Some(fmt_v) = data.get("format-version").and_then(|v| v.as_str()) {
                story.format_version = fmt_v.to_string();
            }
            if let Some(st) = data.get("start").and_then(|v| v.as_str()) {
                story.start_passage = Some(st.to_string());
            }
            if let Some(z) = data.get("zoom").and_then(|v| v.as_f64()) {
                story.zoom = z;
            }
        }
    } else {
        story.passages.push(TweePassage {
            name,
            tags,
            position: pos,
            size,
            body,
        });
    }
    Ok(())
}

pub fn serialize_twee_3(story: &TweeStory) -> String {
    let mut out = String::new();

    out.push_str(":: StoryTitle\n");
    out.push_str(&story.title);
    out.push_str("\n\n");

    out.push_str(":: StoryData\n");
    let mut story_data = json!({
        "ifid": story.ifid,
        "format": story.format,
        "format-version": story.format_version,
        "zoom": story.zoom,
    });
    if let Some(ref start) = story.start_passage {
        story_data["start"] = Value::String(start.clone());
    }
    out.push_str(&serde_json::to_string_pretty(&story_data).unwrap_or_default());
    out.push_str("\n\n");

    for p in &story.passages {
        out.push_str(":: ");
        out.push_str(&p.name);

        if !p.tags.is_empty() {
            out.push_str(" [");
            out.push_str(&p.tags.join(" "));
            out.push(']');
        }

        let mut meta = BTreeMap::new();
        if let Some((x, y)) = p.position {
            meta.insert("position".to_string(), format!("{x},{y}"));
        }
        if let Some((w, h)) = p.size {
            meta.insert("size".to_string(), format!("{w},{h}"));
        }

        if !meta.is_empty() {
            out.push(' ');
            out.push_str(&serde_json::to_string(&meta).unwrap_or_default());
        }

        out.push('\n');
        out.push_str(&p.body);
        out.push_str("\n\n");
    }

    out
}

pub fn serialize_twine_2_html(story: &TweeStory) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>");
    out.push_str(&escape_html(&story.title));
    out.push_str("</title>\n</head>\n<body>\n");

    let startnode = "1";
    out.push_str(&format!(
        "<tw-storydata name=\"{name}\" startnode=\"{startnode}\" creator=\"DecisionNarrative\" creator-version=\"1.0.0\" ifid=\"{ifid}\" zoom=\"{zoom}\" format=\"{format}\" format-version=\"{format_version}\">\n",
        name = escape_html(&story.title),
        startnode = startnode,
        ifid = escape_html(&story.ifid),
        zoom = story.zoom,
        format = escape_html(&story.format),
        format_version = escape_html(&story.format_version)
    ));

    out.push_str("  <style id=\"twine-user-stylesheet\" type=\"text/twine-css\"></style>\n");
    out.push_str("  <script id=\"twine-user-script\" type=\"text/twine-javascript\"></script>\n");

    for (index, p) in story.passages.iter().enumerate() {
        let pid = index + 1;
        let tags = p.tags.join(" ");
        let (x, y) = p.position.unwrap_or((
            (index % 5) as i32 * 160 + 100,
            (index / 5) as i32 * 140 + 100,
        ));
        let (w, h) = p.size.unwrap_or((100, 100));

        out.push_str(&format!(
            "  <tw-passagedata pid=\"{pid}\" name=\"{name}\" tags=\"{tags}\" position=\"{x},{y}\" size=\"{w},{h}\">{body}</tw-passagedata>\n",
            pid = pid,
            name = escape_html(&p.name),
            tags = escape_html(&tags),
            x = x,
            y = y,
            w = w,
            h = h,
            body = escape_html(&p.body)
        ));
    }

    out.push_str("</tw-storydata>\n</body>\n</html>\n");
    out
}

pub fn parse_twine_2_html(html: &str) -> Result<TweeStory, String> {
    let mut story = TweeStory::default();

    let story_re = Regex::new(r#"<tw-storydata\s+([^>]+)>"#).unwrap();
    if let Some(caps) = story_re.captures(html) {
        let attrs = &caps[1];
        if let Some(m) = extract_attr(attrs, "name") {
            story.title = m;
        }
        if let Some(m) = extract_attr(attrs, "ifid") {
            story.ifid = m;
        }
        if let Some(m) = extract_attr(attrs, "format") {
            story.format = m;
        }
        if let Some(m) = extract_attr(attrs, "format-version") {
            story.format_version = m;
        }
        if let Some(m) = extract_attr(attrs, "zoom").and_then(|z| z.parse::<f64>().ok()) {
            story.zoom = m;
        }
    } else {
        return Err("No <tw-storydata> tag found in HTML".into());
    }

    let passage_re = Regex::new(r#"(?s)<tw-passagedata\s+([^>]+)>(.*?)</tw-passagedata>"#).unwrap();
    for cap in passage_re.captures_iter(html) {
        let attrs = &cap[1];
        let body_raw = &cap[2];

        let name = extract_attr(attrs, "name").unwrap_or_else(|| "Untitled".into());
        let tags: Vec<String> = extract_attr(attrs, "tags")
            .map(|t| t.split_whitespace().map(|s| s.to_string()).collect())
            .unwrap_or_default();

        let mut pos = None;
        if let Some(pos_str) = extract_attr(attrs, "position") {
            if let Some((x, y)) = pos_str.split_once(',') {
                if let (Ok(x_i), Ok(y_i)) = (x.trim().parse::<i32>(), y.trim().parse::<i32>()) {
                    pos = Some((x_i, y_i));
                }
            }
        }

        let mut size = None;
        if let Some(size_str) = extract_attr(attrs, "size") {
            if let Some((w, h)) = size_str.split_once(',') {
                if let (Ok(w_i), Ok(h_i)) = (w.trim().parse::<i32>(), h.trim().parse::<i32>()) {
                    size = Some((w_i, h_i));
                }
            }
        }

        let body = unescape_html(body_raw).trim().to_string();

        story.passages.push(TweePassage {
            name,
            tags,
            position: pos,
            size,
            body,
        });
    }

    Ok(story)
}

fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"{}="([^"]*)""#, name);
    let re = Regex::new(&pattern).ok()?;
    re.captures(attrs).map(|c| c[1].to_string())
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn unescape_html(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

pub fn storylets_to_twee_story(project_title: &str, storylets: &[StoryletRecord]) -> TweeStory {
    let mut story = TweeStory::default();
    story.title = project_title.to_string();

    for (index, st) in storylets.iter().enumerate() {
        let mut tags = Vec::new();
        if st.ending {
            tags.push("ending".to_string());
        }
        tags.extend(
            st.location_tags
                .iter()
                .filter(|tag| !tag.trim().is_empty())
                .map(|tag| format!("location_tag:{}", tag.trim())),
        );
        if let Some(ref tag) = st.group_tag {
            if !tag.trim().is_empty() {
                tags.push(tag.trim().to_string());
            }
        }

        let mut body = st
            .content
            .scene_opening
            .clone()
            .unwrap_or_else(|| st.content.text_template.clone());

        if !st.choices.is_empty() {
            body.push_str("\n\n");
            for choice in &st.choices {
                let target = choice.divert_target.as_deref().unwrap_or(&choice.choice_id);
                body.push_str(&format!("[[{}->{}]]\n", choice.label, target));
            }
        }

        let pos = st.position.map(|[x, y]| (x, y)).or_else(|| {
            Some((
                (index % 5) as i32 * 220 + 100,
                (index / 5) as i32 * 180 + 100,
            ))
        });
        let size = st.size.map(|[w, h]| (w, h)).or(Some((120, 100)));

        story.passages.push(TweePassage {
            name: st.title.clone(),
            tags,
            position: pos,
            size,
            body: body.trim().to_string(),
        });
    }

    if let Some(first) = story.passages.first() {
        story.start_passage = Some(first.name.clone());
    }

    story
}

pub fn twee_story_to_storylets(story: &TweeStory) -> Vec<StoryletRecord> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let mut storylets = Vec::new();

    for (index, p) in story.passages.iter().enumerate() {
        let is_ending = p.tags.iter().any(|t| t.eq_ignore_ascii_case("ending"));
        let location_tags: Vec<String> = p
            .tags
            .iter()
            .filter_map(|tag| tag.strip_prefix("location_tag:"))
            .map(str::to_string)
            .collect();
        let group_tag = p
            .tags
            .iter()
            .find(|t| !t.eq_ignore_ascii_case("ending") && !t.starts_with("location_tag:"))
            .cloned();

        let links = extract_twine_links(&p.body);
        let choices = links
            .into_iter()
            .enumerate()
            .map(|(c_idx, l)| StoryletChoiceRecord {
                choice_id: format!("choice_{}", c_idx + 1),
                label: l.display,
                system_verb: Some("ACT".into()),
                conditions: None,
                effects: json!({}),
                divert_target: Some(l.target),
                advisor_preview: None,
            })
            .collect();

        let clean_re = Regex::new(r"\[\[[^\]]+\]\]").unwrap();
        let clean_text = clean_re.replace_all(&p.body, "").trim().to_string();

        let safe_id = format!(
            "stl_twine_{}_{:04}",
            p.name
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric(), "_")
                .trim_matches('_'),
            index + 1
        );

        storylets.push(StoryletRecord {
            id: safe_id,
            revision: 1,
            approved_revision: None,
            title: p.name.clone(),
            ending: is_ending,
            position: p.position.map(|(x, y)| [x, y]),
            size: p.size.map(|(w, h)| [w, h]),
            location_tags,
            group_tag,
            cluster_type: "player_controlled_passage".into(),
            urgency_weight: 50.0,
            repeatability: "always".into(),
            cooldown_turns: 0,
            parameters: Vec::new(),
            preconditions: json!({}),
            content: StoryletContentRecord {
                text_template: clean_text.clone(),
                speaker_id: None,
                scene_opening: Some(clean_text),
                dialogue_quips: None,
            },
            choices,
            effects: json!({}),
            local_variables: None,
            graph: None,
            created_at: now,
        });
    }

    storylets
}
