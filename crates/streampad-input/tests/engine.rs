//! End-to-end tests against real uinput/evdev devices. Ignored by default because they need read/write
//! access to /dev/uinput and create (fake) keyboards; run them with `cargo test -- --include-ignored`.
//! See `common` for the rules that keep every injected event away from the desktop.

mod common;

use common::*;
use std::collections::HashSet;
use std::time::Duration;
use streampad_core::KeyId;
use streampad_core::combo::parse_combo;
use streampad_input::{EngineEvent, KeyAction, ModeSource, UntypableText};

fn pad(key: KeyId, action: KeyAction) -> EngineEvent {
    EngineEvent::PadKey { key, action }
}

fn mode_changed(macro_mode: bool, source: ModeSource) -> EngineEvent {
    EngineEvent::ModeChanged { macro_mode, source }
}

fn expect(events: &crossbeam_channel::Receiver<EngineEvent>, expected: EngineEvent) {
    let found =
        wait_event(events, SETTLE, |e| matches!(e, EngineEvent::ModeChanged { .. } | EngineEvent::PadKey { .. }));
    assert_eq!(found, Some(expected));
}

fn no_pad_events(events: &crossbeam_channel::Receiver<EngineEvent>, what: &str) {
    if let Some(event) = wait_event(events, QUIET, |e| matches!(e, EngineEvent::PadKey { .. })) {
        panic!("{what}: unexpected {event:?}");
    }
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn routes_keys_between_system_and_macropad() {
    let _parallel = run_shared();
    let Rig { engine, events, mirror, keyboard } = Rig::new("Routing");
    let kp4 = KeyId::KP4;

    // (a) Numpad mode: KP4 reaches the mirror, framed exactly like the source reported it.
    keyboard.press(KEY_KP4);
    let scan = 0x70000 + i32::from(KEY_KP4);
    assert_eq!(mirror.next_frame(SETTLE), vec![(EV_MSC, MSC_SCAN, scan), (EV_KEY, KEY_KP4, 1), (EV_SYN, 0, 0)]);
    keyboard.key(KEY_KP4, 2);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 2)), "auto-repeat is forwarded");
    keyboard.release(KEY_KP4);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 0)));
    no_pad_events(&events, "numpad mode");

    // (b) The toggle key switches to macro mode, lights Scroll Lock on the keyboard and is consumed.
    keyboard.drain_leds();
    keyboard.press(KEY_SCROLLLOCK);
    keyboard.key(KEY_SCROLLLOCK, 2);
    keyboard.release(KEY_SCROLLLOCK);
    expect(&events, mode_changed(true, ModeSource::ToggleKey));
    assert!(keyboard.wait_led(LED_SCROLLL, 1, SETTLE), "Scroll Lock LED must show macro mode");
    mirror.assert_no_keys(QUIET, "the toggle key must be consumed");
    assert_no_event(&events, QUIET, |e| matches!(e, EngineEvent::ModeChanged { .. }));
    assert!(engine.macro_mode());

    // LEDs the compositor writes to the mirror reach the keyboard — except Scroll Lock, which shows the mode.
    mirror.write(&[(EV_LED, LED_CAPSL, 1), (EV_SYN, 0, 0)]);
    assert!(keyboard.wait_led(LED_CAPSL, 1, SETTLE), "Caps Lock LED must be forwarded");
    mirror.write(&[(EV_LED, LED_SCROLLL, 1), (EV_SYN, 0, 0), (EV_LED, LED_SCROLLL, 0), (EV_SYN, 0, 0)]);
    assert!(!keyboard.wait_led(LED_SCROLLL, 0, QUIET), "the compositor must not override the macro-mode LED");
    // The compositor also writes LEDs straight to the grabbed keyboard; the macro-mode LED is set again.
    write_to_node(&keyboard.devnode(), &[(EV_LED, LED_SCROLLL, 0), (EV_SYN, 0, 0)]);
    assert!(keyboard.wait_led(LED_SCROLLL, 0, SETTLE));
    assert!(keyboard.wait_led(LED_SCROLLL, 1, SETTLE), "an overwritten macro-mode LED must be restored");

    // (c) Macro mode: KP4 is captured — press, repeat and release go to StreamPad, nothing to the mirror.
    keyboard.press(KEY_KP4);
    keyboard.key(KEY_KP4, 2);
    keyboard.release(KEY_KP4);
    expect(&events, pad(kp4, KeyAction::Press));
    expect(&events, pad(kp4, KeyAction::Repeat));
    expect(&events, pad(kp4, KeyAction::Release));
    mirror.assert_no_keys(QUIET, "captured keys must not reach the system");

    // Non-pad keys always pass through.
    keyboard.press(KEY_A);
    keyboard.release(KEY_A);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_A, 1)));
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_A, 0)));

    // A pad key outside the capture set passes through in macro mode.
    engine.set_capture(HashSet::from([KeyId::KP5]));
    keyboard.press(KEY_KP4);
    keyboard.release(KEY_KP4);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 1)));
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 0)));
    keyboard.press(KEY_KP5);
    keyboard.release(KEY_KP5);
    expect(&events, pad(KeyId::KP5, KeyAction::Press));
    expect(&events, pad(KeyId::KP5, KeyAction::Release));
    engine.set_capture(KeyId::ALL.into_iter().collect());

    // (d) A press passed through in numpad mode keeps its release on the system side…
    engine.set_macro_mode(false);
    expect(&events, mode_changed(false, ModeSource::Api));
    assert!(keyboard.wait_led(LED_SCROLLL, 0, SETTLE));
    keyboard.press(KEY_KP4);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 1)));
    keyboard.press(KEY_SCROLLLOCK);
    keyboard.release(KEY_SCROLLLOCK);
    expect(&events, mode_changed(true, ModeSource::ToggleKey));
    keyboard.release(KEY_KP4);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 0)), "the release must follow its press to the system");
    no_pad_events(&events, "release of a passed-through press");

    // …and a captured press keeps its release on StreamPad's side.
    keyboard.press(KEY_KP4);
    expect(&events, pad(kp4, KeyAction::Press));
    engine.set_macro_mode(false);
    expect(&events, mode_changed(false, ModeSource::Api));
    keyboard.release(KEY_KP4);
    expect(&events, pad(kp4, KeyAction::Release));
    mirror.assert_no_keys(QUIET, "the release of a captured press must not reach the system");

    // Releasing the device hands Scroll Lock back to the compositor's state (it last wrote "off").
    engine.set_macro_mode(true);
    expect(&events, mode_changed(true, ModeSource::Api));
    engine.set_macro_mode(true);
    assert_no_event(&events, QUIET, |e| matches!(e, EngineEvent::ModeChanged { .. }));
    assert!(keyboard.wait_led(LED_SCROLLL, 1, SETTLE));
    keyboard.press(KEY_KP4);
    expect(&events, pad(kp4, KeyAction::Press));
    let mirror_device = mirror_name(&keyboard.name);
    drop(engine);
    expect(&events, pad(kp4, KeyAction::Release));
    assert!(keyboard.wait_led(LED_SCROLLL, 0, SETTLE), "Scroll Lock must be restored on release");
    assert!(wait_until(SETTLE, || !node_exists_named(&mirror_device)), "the mirror must be destroyed");
    drop(mirror);
    // Grabbing succeeds only if the engine really let go of the keyboard.
    let _hidden = Grabbed::open(&keyboard.devnode());
    keyboard.release(KEY_KP4);
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn virtual_keyboard_types_combos_and_text() {
    let _parallel = run_shared();
    let (engine, _events) = start_engine(config_for("SPTEST no such device"));
    let output = engine.output();
    let vk = Grabbed::open(&output.devnode().expect("virtual keyboard node"));
    let combo = |s: &str| parse_combo(s).expect("valid combo");

    // (e) ctrl+shift+t: modifiers first, released in reverse, each event in its own frame, ~2 ms hold.
    output.tap(&combo("ctrl+shift+t")).expect("tap");
    for expected in [(29, 1), (42, 1), (20, 1), (20, 0), (42, 0), (29, 0)] {
        let frame = vk.next_frame(SETTLE);
        assert_eq!(frame, vec![(EV_KEY, expected.0, expected.1), (EV_SYN, 0, 0)]);
    }
    output.tap(&combo("t")).expect("tap");
    let down = vk.next_key_event(SETTLE).expect("press");
    let up = vk.next_key_event(SETTLE).expect("release");
    assert!(up.time - down.time >= Duration::from_micros(1500), "press and release need a gap");

    // Keys held with key_down stay held across taps; release_all lets go of everything.
    output.key_down(&combo("shift")).expect("key_down");
    output.tap(&combo("a")).expect("tap");
    output.key_up(&combo("shift")).expect("key_up");
    output.key_down(&combo("ctrl+alt")).expect("key_down");
    output.release_all();
    let keys: Vec<_> = (0..8).map(|_| vk.next_key(SETTLE).expect("key event")).collect();
    assert_eq!(keys, vec![(42, 1), (30, 1), (30, 0), (42, 0), (29, 1), (56, 1), (56, 0), (29, 0)]);

    // Text holds Shift for uppercase letters and symbols.
    output.type_text("Hi!", Duration::from_millis(1)).expect("type_text");
    let keys: Vec<_> = (0..10).map(|_| vk.next_key(SETTLE).expect("key event")).collect();
    assert_eq!(keys, vec![(42, 1), (35, 1), (35, 0), (42, 0), (23, 1), (23, 0), (42, 1), (2, 1), (2, 0), (42, 0)]);

    // Untypable text fails before typing anything.
    let err = output.type_text("naïve", Duration::ZERO).expect_err("ï is not on the US layout");
    assert_eq!(err.downcast_ref::<UntypableText>().map(|e| e.0.as_str()), Some("ï"));
    vk.assert_no_keys(QUIET, "nothing may be typed when the text is untypable");

    // Concurrent macros never interleave inside one combo. (Read while they type: the evdev client
    // buffer would overflow otherwise.)
    let combos = ["ctrl+a", "alt+b", "meta+c", "shift+d"];
    let expected: Vec<Vec<u16>> = combos.iter().map(|name| combo(name).keys).collect();
    std::thread::scope(|scope| {
        for name in combos {
            let output = output.clone();
            let parsed = combo(name);
            scope.spawn(move || (0..25).for_each(|_| output.tap(&parsed).expect("tap")));
        }
        for _ in 0..combos.len() * 25 {
            let chunk: Vec<_> = (0..4).map(|_| vk.next_key(SETTLE).expect("key event")).collect();
            let keys = expected.iter().find(|keys| keys[0] == chunk[0].0);
            let keys = keys.unwrap_or_else(|| panic!("combo starts with an unexpected key: {chunk:?}"));
            assert_eq!(chunk, vec![(keys[0], 1), (keys[1], 1), (keys[1], 0), (keys[0], 0)], "interleaved combo");
        }
    });
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn hotplugged_keyboards_are_grabbed_and_released() {
    let _parallel = run_shared();
    // (f) The engine starts before the keyboard exists.
    let name = random_name("Hotplug");
    let (engine, events) = start_engine(config_for(&name));
    assert!(engine.devices().is_empty());

    let keyboard = FakeKeyboard::create(&name);
    let grabbed = |e: &EngineEvent| matches!(e, EngineEvent::DevicesChanged(list) if list.iter().any(|d| d.name == name && d.grabbed));
    assert!(wait_event(&events, SETTLE, grabbed).is_some(), "a hotplugged keyboard must be grabbed");
    let info = engine.devices().into_iter().find(|d| d.name == name).expect("listed");
    assert_eq!(info.id, format!("1209:5350:{name}"));
    assert!(info.has_numpad && info.eligible && info.error.is_none());
    assert_eq!(info.path, keyboard.devnode().to_string_lossy());

    let mirror = Grabbed::open(&find_node(&mirror_name(&name)));
    keyboard.press(KEY_KP4);
    keyboard.release(KEY_KP4);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 1)));
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 0)));

    // Unplugging tears the mirror down and drops the device from the list.
    drop(keyboard);
    let gone = |e: &EngineEvent| matches!(e, EngineEvent::DevicesChanged(list) if list.iter().all(|d| d.name != name));
    assert!(wait_event(&events, SETTLE, gone).is_some(), "an unplugged keyboard must disappear");
    assert!(wait_until(SETTLE, || !node_exists_named(&mirror_name(&name))), "its mirror must be destroyed");
    assert!(engine.devices().is_empty());

    // Plugging it in again works the same.
    let again = FakeKeyboard::create(&name);
    assert!(wait_until(SETTLE, || is_grabbed(&engine, &name)), "a re-plugged keyboard must be grabbed again");
    drop(engine);
    drop(again);
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn reconfigure_moves_the_grab() {
    let _parallel = run_shared();
    let first = FakeKeyboard::create(&random_name("First"));
    let second = FakeKeyboard::create(&random_name("Second"));
    let (engine, _events) = start_engine(config_for(&first.name));
    assert!(wait_until(SETTLE, || is_grabbed(&engine, &first.name)));
    assert!(!is_grabbed(&engine, &second.name));

    engine.reconfigure(config_for(&second.name));
    assert!(wait_until(SETTLE, || is_grabbed(&engine, &second.name)), "the newly selected keyboard is grabbed");
    assert!(wait_until(SETTLE, || !node_exists_named(&mirror_name(&first.name))), "the old mirror is destroyed");
    assert!(engine.devices().iter().all(|d| d.name != first.name));
    drop(Grabbed::open(&first.devnode()));
    drop(engine);
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn grab_waits_until_held_keys_are_released() {
    let _parallel = run_shared();
    // (g) Hold Shift before the engine starts. Our own grab hides the press from the desktop, so the later
    // release (sent while nobody grabs the keyboard) is ignored by the compositor.
    let keyboard = FakeKeyboard::create(&random_name("Held"));
    {
        let hidden = Grabbed::open(&keyboard.devnode());
        keyboard.press(KEY_LEFTSHIFT);
        assert_eq!(hidden.next_key(SETTLE), Some((KEY_LEFTSHIFT, 1)));
    }
    let (engine, _events) = start_engine(config_for(&keyboard.name));
    std::thread::sleep(Duration::from_millis(800));
    assert!(!is_grabbed(&engine, &keyboard.name), "must not grab while a key is held");

    keyboard.release(KEY_LEFTSHIFT);
    assert!(wait_until(SETTLE, || is_grabbed(&engine, &keyboard.name)), "grab once the key is released");
    let mirror = Grabbed::open(&find_node(&mirror_name(&keyboard.name)));
    mirror.assert_no_keys(QUIET, "events from before the grab must not be forwarded");
    keyboard.press(KEY_KP4);
    keyboard.release(KEY_KP4);
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 1)));
    assert_eq!(mirror.next_key(SETTLE), Some((KEY_KP4, 0)));
    drop(engine);
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn failed_grab_is_reported_and_retried_on_reconfigure() {
    let _parallel = run_shared();
    // Another program (the test) holds the grab, so the engine's grab fails.
    let keyboard = FakeKeyboard::create(&random_name("Busy"));
    let other_program = Grabbed::open(&keyboard.devnode());
    let (engine, events) = start_engine(config_for(&keyboard.name));
    let warning = wait_event(&events, SETTLE, |e| matches!(e, EngineEvent::Warning(_)));
    assert!(matches!(&warning, Some(EngineEvent::Warning(text)) if text.contains("another program has grabbed")));
    let info = engine.devices().into_iter().find(|d| d.name == keyboard.name).expect("listed");
    assert!(!info.grabbed);
    assert_eq!(info.error.as_deref(), Some("another program has grabbed this device"));
    assert!(wait_until(SETTLE, || !node_exists_named(&mirror_name(&keyboard.name))), "the mirror is destroyed");

    // Not retried on its own…
    std::thread::sleep(Duration::from_millis(500));
    drop(other_program);
    assert!(!is_grabbed(&engine, &keyboard.name));
    // …but on reconfigure.
    engine.reconfigure(config_for(&keyboard.name));
    assert!(wait_until(SETTLE, || is_grabbed(&engine, &keyboard.name)), "reconfigure retries failed devices");
    let info = engine.devices().into_iter().find(|d| d.name == keyboard.name).expect("listed");
    assert_eq!(info.error, None);
    drop(engine);
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn grab_timeout_releases_everything() {
    let _parallel = run_shared();
    let rig = Rig::with_config("Timeout", |cfg| {
        cfg.grab_timeout = Some(Duration::from_secs(2));
        cfg.macro_mode = true;
    });
    rig.keyboard.press(KEY_KP4);
    expect(&rig.events, pad(KeyId::KP4, KeyAction::Press));
    let stopped = wait_event(&rig.events, Duration::from_secs(6), |e| {
        matches!(e, EngineEvent::Stopped { .. } | EngineEvent::PadKey { action: KeyAction::Release, .. })
    });
    assert_eq!(stopped, Some(pad(KeyId::KP4, KeyAction::Release)), "held captured keys are released first");
    let stopped = wait_event(&rig.events, SETTLE, |e| matches!(e, EngineEvent::Stopped { .. }));
    assert!(matches!(stopped, Some(EngineEvent::Stopped { reason }) if reason.contains("grab timeout")));
    assert!(!is_grabbed(&rig.engine, &rig.keyboard.name));
    assert!(wait_until(SETTLE, || !node_exists_named(&mirror_name(&rig.keyboard.name))));
    let _hidden = Grabbed::open(&rig.keyboard.devnode());
    rig.keyboard.release(KEY_KP4);
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn passthrough_latency() {
    let _alone = run_exclusive();
    // (h) Round trip per event: written to the fake keyboard → read back from the mirror. Latency is from
    // just before the write until the test has read the event, so it also includes the test's own wakeup
    // (the mirror's kernel timestamp can't be used: uinput keeps the source event's timestamp).
    let rig = Rig::new("Latency");
    const EVENTS: usize = 5000;
    let mut samples = Vec::with_capacity(EVENTS);
    for i in 0..EVENTS {
        let value = i32::from(i % 2 == 0);
        let sent = monotonic_now();
        rig.keyboard.key(KEY_A, value);
        let event = rig.mirror.next_key_event(SETTLE).expect("forwarded");
        assert_eq!((event.code, event.value), (KEY_A, value));
        // Event timestamps have microsecond resolution.
        let source_time = event.time + Duration::from_micros(1);
        assert!(source_time >= sent && event.time <= event.received, "the mirror keeps the source timestamp");
        samples.push(event.received.saturating_sub(sent));
    }
    samples.sort_unstable();
    let mean = samples.iter().sum::<Duration>() / EVENTS as u32;
    let percentile = |p: usize| samples[(EVENTS * p / 100).min(EVENTS - 1)];
    println!(
        "passthrough latency over {EVENTS} events: mean {mean:?}, p50 {:?}, p99 {:?}, max {:?}",
        percentile(50),
        percentile(99),
        samples[EVENTS - 1]
    );
    assert!(percentile(99) < Duration::from_millis(1), "p99 latency {:?} exceeds 1 ms", percentile(99));
}
