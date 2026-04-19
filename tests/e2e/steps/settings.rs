//! Step definitions for the Settings feature.
//!
//! See `plan/10-settings.md` section 11.1.

use blockexplorer_tui::{
    adapters::ui::{AppConfigSnapshot, PalettePreset, ScreenStack, Screen, SettingsScreen},
    domain::Chain,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cucumber::{then, when};

use crate::{steps::search::build_stack, world::AppWorld};

fn current(stack: &ScreenStack) -> &SettingsScreen {
    stack
        .top()
        .expect("stack non-empty")
        .as_any()
        .downcast_ref::<SettingsScreen>()
        .expect("top of stack must be a SettingsScreen")
}

fn current_mut(stack: &mut ScreenStack) -> &mut SettingsScreen {
    stack
        .top_mut()
        .expect("stack non-empty")
        .as_any_mut()
        .downcast_mut::<SettingsScreen>()
        .expect("top of stack must be a SettingsScreen")
}

fn push_settings(world: &mut AppWorld, alchemy_key_present: bool) {
    build_stack(world);
    let chain = *world.active_chain.get_or_insert(Chain::Ethereum);
    let snapshot = AppConfigSnapshot {
        chain,
        alchemy_key_present,
        config_path_hint: Some("/tmp/test-config.toml".to_string()),
    };
    let stack = world.stack.as_mut().unwrap();
    stack.push(Box::new(SettingsScreen::new(snapshot)));
}

#[when("the user opens Settings with an alchemy key configured")]
async fn opens_settings_with_key(world: &mut AppWorld) {
    push_settings(world, true);
}

#[when("the user opens Settings with no alchemy key")]
async fn opens_settings_without_key(world: &mut AppWorld) {
    push_settings(world, false);
}

#[then("the Settings screen reports the alchemy key as configured")]
async fn reports_configured(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    assert!(current(stack).snapshot().alchemy_key_present);
}

#[then("the Settings screen reports the alchemy key as missing")]
async fn reports_missing(world: &mut AppWorld) {
    let stack = world.stack.as_ref().expect("stack");
    assert!(!current(stack).snapshot().alchemy_key_present);
}

#[when(regex = r#"^the user presses "(\d)" on Settings$"#)]
async fn press_digit_on_settings(world: &mut AppWorld, digit: String) {
    let stack = world.stack.as_mut().expect("stack");
    let screen = current_mut(stack);
    let ch = digit.chars().next().expect("digit char");
    screen.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
}

#[then(regex = r#"^the Settings screen reports "([^"]+)" as the active palette$"#)]
async fn palette_is(world: &mut AppWorld, label: String) {
    let stack = world.stack.as_ref().expect("stack");
    let screen = current(stack);
    let expected = PalettePreset::all()
        .into_iter()
        .find(|p| p.label() == label)
        .unwrap_or_else(|| panic!("scenario references unknown palette label: {label}"));
    assert_eq!(screen.palette(), expected);
}
