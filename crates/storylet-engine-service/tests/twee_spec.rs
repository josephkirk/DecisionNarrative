use storylet_engine_service::contracts::{
    StoryletChoiceRecord, StoryletContentRecord, StoryletRecord,
};
use storylet_engine_service::twee::{
    TweePassage, TweeStory, extract_twine_links, parse_twee_3, parse_twine_2_html,
    serialize_twee_3, serialize_twine_2_html, storylets_to_twee_story, twee_story_to_storylets,
};

#[test]
fn test_twine_link_extraction() {
    let text = "Here is [[Simple]], and [[Go North->NorthRoom]], plus [[SouthRoom<-Head South]], and [[Look West|WestRoom]].";
    let links = extract_twine_links(text);
    assert_eq!(links.len(), 4);
    assert_eq!(links[0].display, "Simple");
    assert_eq!(links[0].target, "Simple");

    assert_eq!(links[1].display, "Go North");
    assert_eq!(links[1].target, "NorthRoom");

    assert_eq!(links[2].display, "Head South");
    assert_eq!(links[2].target, "SouthRoom");

    assert_eq!(links[3].display, "Look West");
    assert_eq!(links[3].target, "WestRoom");
}

#[test]
fn test_twee_3_roundtrip() {
    let input = ":: StoryTitle\nMy Adventure\n\n:: StoryData\n{\n  \"format\": \"SugarCube\",\n  \"format-version\": \"2.36.1\",\n  \"ifid\": \"D674C58C-DEFA-4F70-B7A2-27742230C0FC\",\n  \"start\": \"Start\",\n  \"zoom\": 1.0\n}\n\n:: Start [intro] {\"position\":\"100,100\",\"size\":\"100,100\"}\nYou wake up in a damp cellar.\n\n[[Look around->CellarLook]]\n\n:: CellarLook [ending] {\"position\":\"300,100\",\"size\":\"100,100\"}\nYou find an iron key.\n";

    let story = parse_twee_3(input).unwrap();
    assert_eq!(story.title, "My Adventure");
    assert_eq!(story.ifid, "D674C58C-DEFA-4F70-B7A2-27742230C0FC");
    assert_eq!(story.passages.len(), 2);
    assert_eq!(story.passages[0].name, "Start");
    assert_eq!(story.passages[0].tags, vec!["intro"]);
    assert_eq!(story.passages[0].position, Some((100, 100)));

    let serialized = serialize_twee_3(&story);
    let story2 = parse_twee_3(&serialized).unwrap();
    assert_eq!(story2.title, story.title);
    assert_eq!(story2.passages.len(), story.passages.len());
    assert_eq!(story2.passages[0].name, story.passages[0].name);
}

#[test]
fn test_twine_2_html_roundtrip() {
    let mut story = TweeStory::default();
    story.title = "HTML Twine Game".into();
    story.passages.push(TweePassage {
        name: "First Passage".into(),
        tags: vec!["hub".into()],
        position: Some((120, 150)),
        size: Some((100, 100)),
        body: "Text with [[Next Room->SecondPassage]].".into(),
    });

    let html = serialize_twine_2_html(&story);
    assert!(html.contains("<tw-storydata"));
    assert!(html.contains("<tw-passagedata"));
    assert!(html.contains("First Passage"));

    let parsed = parse_twine_2_html(&html).unwrap();
    assert_eq!(parsed.title, "HTML Twine Game");
    assert_eq!(parsed.passages.len(), 1);
    assert_eq!(parsed.passages[0].name, "First Passage");
    assert_eq!(parsed.passages[0].position, Some((120, 150)));
}
