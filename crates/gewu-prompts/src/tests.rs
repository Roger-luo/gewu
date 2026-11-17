use std::path::Path;

use super::{Prompt, addenda::ADDENDA, compose};

const PROMPTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../harness/prompts");

#[test]
fn every_prompt_file_is_in_the_catalog() {
    for entry in std::fs::read_dir(PROMPTS).expect("prompts directory") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|ext| ext == "md") {
            let stem = path.file_stem().and_then(|s| s.to_str()).expect("stem");
            assert!(
                Prompt::find(stem).is_some(),
                "{stem}.md is not in the catalog"
            );
        }
    }
}

#[test]
fn every_backend_addition_on_disk_is_registered() {
    let backends = Path::new(PROMPTS).join("backends");
    for preset in std::fs::read_dir(&backends).expect("backends directory") {
        let preset = preset.expect("entry").path();
        if !preset.is_dir() {
            continue;
        }
        let name = preset
            .file_name()
            .and_then(|s| s.to_str())
            .expect("name")
            .to_owned();
        for file in std::fs::read_dir(&preset).expect("preset directory") {
            let file = file.expect("entry").path();
            let stem = file.file_stem().and_then(|s| s.to_str()).expect("stem");
            let prompt = Prompt::find(stem)
                .unwrap_or_else(|| panic!("backends/{name}/{stem}.md names no prompt"));
            assert!(
                prompt.addition(&name).is_some(),
                "backends/{name}/{stem}.md is not registered in addenda.rs"
            );
        }
    }
    for (preset, prompt, _) in ADDENDA {
        assert!(
            Prompt::ALL.contains(prompt),
            "{preset} adds to an unknown prompt"
        );
    }
}

#[test]
fn a_backend_without_an_addition_gets_the_general_prompt() {
    let general = Prompt::Expand.general();
    assert_eq!(Prompt::Expand.text_for(None), general);
    assert_eq!(Prompt::Expand.text_for(Some("no-such-agent")), general);
}

#[test]
fn an_addition_follows_the_general_prompt() {
    let text = compose("General rules.\n\n", "\nFor this model: one more.\n");
    assert_eq!(text, "General rules.\n\nFor this model: one more.\n");
}

#[test]
fn every_prompt_says_what_it_is_for() {
    for prompt in Prompt::ALL {
        assert!(
            !prompt.general().trim().is_empty(),
            "{} is empty",
            prompt.id()
        );
        assert!(
            prompt.when().ends_with('.'),
            "{} needs a sentence",
            prompt.id()
        );
    }
}
