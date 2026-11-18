//! The reviewer's workbench settings: saved in the home, shown in the
//! snapshot, and checked before they are kept.

mod support;

use gewu::workbench::{AgentChoice, Home, Settings, Workbench, WorkbenchAction, WriteAhead};
use support::Fixture;

#[test]
fn settings_are_saved_and_outlive_the_run() {
    let fixture = Fixture::new();
    let defaults = fixture.workbench.snapshot().expect("snapshot").settings;
    assert_eq!(defaults.settings, Settings::default());
    assert_eq!(defaults.settings.proofread_rounds, 2);
    // Agents write faster than people read: by default it writes on.
    assert_eq!(defaults.settings.write_ahead, WriteAhead::Unlimited);
    assert!(defaults.home.ends_with("home"), "{}", defaults.home);

    let chosen = Settings {
        proofread_rounds: 0,
        default_agent: Some(AgentChoice {
            preset_id: "cursor".into(),
            command: None,
        }),
        explain_tests: true,
        proofreader: Some("claude".into()),
        write_ahead: WriteAhead::Batches(3),
    };
    let snapshot = fixture
        .workbench
        .dispatch(WorkbenchAction::UpdateSettings {
            settings: chosen.clone(),
        })
        .expect("save");
    assert_eq!(snapshot.settings.settings, chosen);

    let reopened = Workbench::open(
        Home::new(&defaults.home),
        std::path::Path::new(&defaults.gewu),
    )
    .expect("reopen");
    assert_eq!(reopened.settings(), chosen);
}

#[test]
fn out_of_range_settings_are_refused() {
    let fixture = Fixture::new();
    for settings in [
        Settings {
            proofread_rounds: 99,
            ..Settings::default()
        },
        Settings {
            proofread_rounds: 1,
            default_agent: Some(AgentChoice {
                preset_id: "custom".into(),
                command: Some("  ".into()),
            }),
            ..Settings::default()
        },
        // Neither starts an agent that can proofread.
        Settings {
            proofreader: Some("none".into()),
            ..Settings::default()
        },
        Settings {
            proofreader: Some("custom".into()),
            ..Settings::default()
        },
        // Past the most batches; no limit says that plainly.
        Settings {
            write_ahead: WriteAhead::Batches(WriteAhead::MAX_BATCHES + 1),
            ..Settings::default()
        },
    ] {
        let error = fixture
            .workbench
            .dispatch(WorkbenchAction::UpdateSettings { settings })
            .expect_err("refused");
        assert!(error.is_client_error(), "{error}");
    }
    assert_eq!(fixture.workbench.settings(), Settings::default());
}
