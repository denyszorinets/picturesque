use super::*;

const PRESETS: [CriticPreset; 3] = [
    CriticPreset::Professional,
    CriticPreset::Amateur,
    CriticPreset::Beginner,
];

#[test]
fn the_prompts_and_keys_are_pinned_so_a_change_is_deliberate() {
    let prompt = system_prompt(CriticPreset::Professional, DEFAULT_RUBRIC);
    assert_eq!(
        (fingerprint(&prompt), prompt.len()),
        ("ce6629f22d0e608f".into(), 5175)
    );
    assert_eq!(
        rubric_key(CriticPreset::Professional, DEFAULT_RUBRIC),
        "c80a69cb2fbe62ca"
    );
    // Other presets: the key is built from the serialized name, so it stays
    // stable if the Rust variant is renamed. Professional hashes only the
    // version and the rubric, as it did before presets.
    assert_eq!(
        rubric_key(CriticPreset::Amateur, DEFAULT_RUBRIC),
        "9597f97a3e264f4a"
    );
}

#[test]
fn as_str_matches_the_serialized_name() {
    for preset in PRESETS {
        assert_eq!(
            serde_json::to_string(&preset).unwrap(),
            format!("\"{}\"", preset.as_str())
        );
    }
}

#[test]
fn each_preset_has_its_voice_and_the_shared_schema_instructions() {
    // (preset, phrase only it has)
    let cases = [
        (CriticPreset::Professional, "strict juror"),
        (
            CriticPreset::Amateur,
            "demanding but fair photography coach",
        ),
        (CriticPreset::Beginner, "encouraging photography teacher"),
    ];
    for (preset, phrase) in cases {
        let prompt = system_prompt(preset, "MY RUBRIC");
        assert!(prompt.contains(phrase), "{preset:?}: {phrase}");
        for shared in [
            "MY RUBRIC",
            "Look hard for flaws first",
            "\"weaknesses\"",
            "\"potential\"",
            "\"fixable\"",
            "\"fix_steps\"",
            "\"improvements\"",
            "name the setting to change",
            "\"camera_tips\"",
            "Reply with JSON only.",
        ] {
            assert!(prompt.contains(shared), "{preset:?}: {shared}");
        }
        // Only its own voice.
        for other in PRESETS.into_iter().filter(|p| *p != preset) {
            assert_ne!(prompt, system_prompt(other, "MY RUBRIC"));
        }
    }
    assert!(system_prompt(CriticPreset::Beginner, "r").contains("raise the ISO a little"));
}

#[test]
fn every_preset_states_the_photography_facts_the_tips_rest_on() {
    for preset in PRESETS {
        let prompt = system_prompt(preset, "r");
        for fact in [
            "SMALLER f-number (f/1.8) opens the lens wider",
            "LARGER f-number (f/8, f/11) lets in less light and gives MORE depth of field",
            "Never suggest a value the shot already used",
        ] {
            assert!(prompt.contains(fact), "{preset:?}: {fact}");
        }
    }
}

#[test]
fn the_rubric_key_tells_the_presets_and_rubrics_apart() {
    let mut keys = Vec::new();
    for preset in PRESETS {
        for rubric in [DEFAULT_RUBRIC, "Only the light."] {
            keys.push(rubric_key(preset, rubric));
        }
    }
    let unique: std::collections::HashSet<_> = keys.iter().collect();
    assert_eq!(unique.len(), keys.len());
}
