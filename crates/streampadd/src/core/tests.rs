//! State-machine tests with in-process fakes for the input engine and the macro runtime. Time is explicit:
//! every call passes `now`, so timer behaviour is tested exactly.

use super::*;
use crate::backend::{OnDone, RunControl};
use serde_json::json;
use std::sync::Mutex;
use streampad_core::ipc::{RenderKeyKind, events};
use streampad_core::{Concurrency, GlobalAction, GlobalBinding, Macro};
use streampad_input::{EngineEvent, KeyAction, ModeSource};
use streampad_script::Invocation;

#[derive(Default)]
struct FakeInput {
    modes: Mutex<Vec<bool>>,
    captures: Mutex<Vec<HashSet<KeyId>>>,
    reconfigured: Mutex<usize>,
}

impl InputBackend for FakeInput {
    fn set_macro_mode(&self, on: bool) {
        self.modes.lock().unwrap().push(on);
    }
    fn set_capture(&self, keys: HashSet<KeyId>) {
        self.captures.lock().unwrap().push(keys);
    }
    fn reconfigure(&self, _cfg: streampad_input::EngineConfig) {
        *self.reconfigured.lock().unwrap() += 1;
    }
    fn devices(&self) -> Vec<DeviceInfo> {
        Vec::new()
    }
    fn shutdown(&self) {}
}

/// What happened to one spawned run.
#[derive(Default)]
struct RunLog {
    cancelled: bool,
    released: usize,
}

#[derive(Default)]
struct FakeRunner {
    spawned: Mutex<Vec<(Invocation, String)>>,
    runs: Arc<Mutex<HashMap<u64, RunLog>>>,
    library: Mutex<String>,
}

struct FakeRun {
    id: u64,
    runs: Arc<Mutex<HashMap<u64, RunLog>>>,
}

impl RunControl for FakeRun {
    fn cancel(&self) {
        self.runs.lock().unwrap().entry(self.id).or_default().cancelled = true;
    }
    fn release(&self) {
        self.runs.lock().unwrap().entry(self.id).or_default().released += 1;
    }
}

impl MacroRunner for FakeRunner {
    fn set_library(&self, source: String) {
        *self.library.lock().unwrap() = source;
    }
    fn spawn(&self, code: Arc<CompiledMacro>, inv: Invocation, _on_done: OnDone) -> Box<dyn RunControl> {
        let id = inv.run_id;
        self.runs.lock().unwrap().insert(id, RunLog::default());
        self.spawned.lock().unwrap().push((inv, code.code.clone()));
        Box::new(FakeRun { id, runs: self.runs.clone() })
    }
    fn check_syntax(&self, _source: &str) -> Result<(), String> {
        Ok(())
    }
}

fn script(source: &str) -> Option<Macro> {
    Some(Macro::Script { source: source.into() })
}

fn bind<'a>(cfg: &'a mut Config, profile: usize, key: KeyId, source: &str) -> &'a mut Binding {
    let b = cfg.profiles[profile].keys.entry(key).or_default();
    b.label = format!("{key} macro");
    b.r#macro = script(source);
    b
}

/// Three profiles (A, B, C); A has KP5 bound. Default globals.
fn test_config() -> Config {
    let mut cfg = Config { profiles: vec![Profile::new("A"), Profile::new("B"), Profile::new("C")], ..Config::default() };
    bind(&mut cfg, 0, KeyId::KP5, "log('five')");
    cfg
}

struct Harness {
    core: Core,
    input: Arc<FakeInput>,
    runner: Arc<FakeRunner>,
    t0: Instant,
}

impl Harness {
    fn new(cfg: Config) -> Harness {
        Self::with_initial(cfg, |_| {})
    }

    fn with_initial(cfg: Config, tweak: impl FnOnce(&mut Initial)) -> Harness {
        let input = Arc::new(FakeInput::default());
        let runner = Arc::new(FakeRunner::default());
        let paths = Arc::new(Paths {
            config_dir: "/nonexistent/config".into(),
            data_dir: "/nonexistent/data".into(),
            state_dir: "/nonexistent/state".into(),
            runtime_dir: "/nonexistent/run".into(),
            socket: "/nonexistent/run/daemon.sock".into(),
            share_dir: None,
        });
        let deps = Deps {
            paths,
            input: input.clone(),
            runner: runner.clone(),
            on_run_done: Arc::new(|_, _| {}),
            overrides: InputOverrides::default(),
        };
        let mut initial = Initial::new(&cfg, None);
        tweak(&mut initial);
        let t0 = Instant::now();
        let mut core = Core::new(cfg, None, initial, deps, t0);
        core.take_effects();
        Harness { core, input, runner, t0 }
    }

    fn at(&self, ms: u64) -> Instant {
        self.t0 + Duration::from_millis(ms)
    }

    fn macro_mode_on(&mut self, ms: u64) {
        let now = self.at(ms);
        self.core.engine_event(EngineEvent::ModeChanged { macro_mode: true, source: ModeSource::ToggleKey }, now);
    }

    fn key(&mut self, key: KeyId, action: KeyAction, ms: u64) {
        let now = self.at(ms);
        self.core.pad_key(key, action, now);
    }

    fn tap(&mut self, key: KeyId, ms: u64) {
        self.key(key, KeyAction::Press, ms);
        self.key(key, KeyAction::Release, ms + 50);
    }

    fn tick(&mut self, ms: u64) {
        let now = self.at(ms);
        self.core.tick(now);
    }

    fn events(&mut self, name: &str) -> Vec<Value> {
        self.core
            .take_effects()
            .into_iter()
            .filter_map(|e| match e {
                Effect::Event(ev) if ev.name == name => Some(ev.data),
                _ => None,
            })
            .collect()
    }

    fn visible(&mut self) -> bool {
        self.core.snapshot().overlay.visible
    }

    fn spawned(&self) -> Vec<u64> {
        self.runner.spawned.lock().unwrap().iter().map(|(inv, _)| inv.run_id).collect()
    }

    fn run_log(&self, id: u64) -> (bool, usize) {
        let runs = self.runner.runs.lock().unwrap();
        runs.get(&id).map_or((false, 0), |r| (r.cancelled, r.released))
    }

    fn request(&mut self, method: &str, params: Value, ms: u64) -> Result<Value, ErrorBody> {
        let now = self.at(ms);
        self.core.request(1, method, params, now)
    }
}

use streampad_core::ipc::ErrorBody;

// ---- globals ---------------------------------------------------------------------------------------------

#[test]
fn global_without_hold_fires_on_press_and_wraps() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.key(KeyId::KPPlus, KeyAction::Press, 10);
    assert_eq!(h.core.profile_index, 1, "tap fires on press when there is no hold action");
    h.key(KeyId::KPPlus, KeyAction::Release, 40);
    h.tap(KeyId::KPPlus, 100);
    h.tap(KeyId::KPPlus, 200);
    assert_eq!(h.core.profile_index, 0, "next wraps from the last profile to the first");
    h.tap(KeyId::KPMinus, 300);
    assert_eq!(h.core.profile_index, 2, "prev wraps from the first profile to the last");
}

#[test]
fn global_with_hold_taps_on_release_or_holds_at_hold_ms() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.core.take_effects();
    let pinned = |h: &Harness| h.core.overlay.pinned;

    h.key(KeyId::KPEnter, KeyAction::Press, 1000);
    assert!(!pinned(&h), "tap waits for the release when a hold action exists");
    assert_eq!(h.core.next_deadline(), Some(h.at(1600)));
    h.key(KeyId::KPEnter, KeyAction::Release, 1300);
    assert!(pinned(&h), "released before holdMs = tap");
    assert!(!h.core.take_effects().contains(&Effect::OpenEditor));

    h.key(KeyId::KPEnter, KeyAction::Press, 2000);
    h.tick(2599);
    assert!(!h.core.take_effects().contains(&Effect::OpenEditor));
    h.tick(2600);
    assert!(h.core.take_effects().contains(&Effect::OpenEditor), "hold fires as soon as holdMs elapses");
    h.key(KeyId::KPEnter, KeyAction::Release, 3000);
    assert!(pinned(&h), "the release after a hold does not tap");
    assert!(!h.core.take_effects().contains(&Effect::OpenEditor));
}

#[test]
fn hold_threshold_follows_settings() {
    let mut cfg = test_config();
    cfg.settings.hold_ms = 300;
    let mut h = Harness::new(cfg);
    h.macro_mode_on(0);
    h.key(KeyId::KPEnter, KeyAction::Press, 100);
    h.tick(400);
    assert!(h.core.take_effects().contains(&Effect::OpenEditor));
}

#[test]
fn auto_repeat_is_ignored_for_globals() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.key(KeyId::KPPlus, KeyAction::Press, 10);
    for ms in [300, 330, 360] {
        h.key(KeyId::KPPlus, KeyAction::Repeat, ms);
    }
    h.key(KeyId::KPPlus, KeyAction::Release, 400);
    assert_eq!(h.core.profile_index, 1);
}

#[test]
fn leaving_macro_mode_cancels_a_pending_global() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.key(KeyId::KPEnter, KeyAction::Press, 100);
    h.core.engine_event(EngineEvent::ModeChanged { macro_mode: false, source: ModeSource::ToggleKey }, h.at(200));
    h.tick(1000);
    h.key(KeyId::KPEnter, KeyAction::Release, 1100);
    assert!(!h.core.overlay.pinned);
    assert!(!h.core.take_effects().contains(&Effect::OpenEditor));
}

#[test]
fn global_key_events_are_emitted() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.core.take_effects();
    h.tap(KeyId::KPPlus, 10);
    let keys = h.events(events::KEY);
    assert_eq!(keys, [json!({"key":"KPPlus","pressed":true,"kind":"global"}), json!({"key":"KPPlus","pressed":false,"kind":"global"})]);
}

// ---- overlay ---------------------------------------------------------------------------------------------

#[test]
fn peek_on_mode_change_then_auto_hide() {
    let mut h = Harness::new(test_config());
    assert!(!h.visible());
    h.macro_mode_on(0);
    assert!(h.visible(), "entering macro mode peeks");
    assert_eq!(h.core.next_deadline(), Some(h.at(1600)));
    h.tick(1599);
    assert!(h.visible());
    h.tick(1600);
    assert!(!h.visible(), "peek hides after peekMs");
}

#[test]
fn peek_on_profile_change_restarts_the_timer() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.tick(1700);
    assert!(!h.visible());
    h.tap(KeyId::KPPlus, 2000);
    assert!(h.visible());
    h.tick(3599);
    assert!(h.visible());
    h.tick(3600);
    assert!(!h.visible());
}

#[test]
fn pinned_survives_leaving_macro_mode() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.tap(KeyId::KPEnter, 100);
    h.tick(5000);
    assert!(h.visible(), "pinned stays after the peek");
    h.core.engine_event(EngineEvent::ModeChanged { macro_mode: false, source: ModeSource::ToggleKey }, h.at(6000));
    assert!(!h.visible(), "leaving macro mode hides");
    assert!(h.core.persisted().pinned, "but remembers the pin");
    h.macro_mode_on(7000);
    h.tick(9000);
    assert!(h.visible(), "re-entering shows it again");
    h.tap(KeyId::KPEnter, 9100);
    assert!(!h.visible());
}

#[test]
fn peek_settings_can_disable_peeking() {
    let mut cfg = test_config();
    cfg.settings.overlay.peek_on_mode_change = false;
    cfg.settings.overlay.peek_on_profile_change = false;
    let mut h = Harness::new(cfg);
    h.macro_mode_on(0);
    assert!(!h.visible());
    h.tap(KeyId::KPPlus, 100);
    assert!(!h.visible());
}

#[test]
fn preview_shows_outside_macro_mode() {
    let mut h = Harness::new(test_config());
    let r = h.request("overlay", json!({ "action": "toggle", "preview": true }), 0).unwrap();
    assert_eq!(r, json!({ "visible": true }));
    let s = h.core.snapshot();
    assert!(s.overlay.visible && s.overlay.preview && !s.macro_mode && !s.overlay.pinned);
    h.request("overlay", json!({ "action": "toggle", "preview": true }), 10).unwrap();
    assert!(!h.visible());
    h.request("overlay", json!({ "action": "peek", "preview": true, "durationMs": 500 }), 20).unwrap();
    assert!(h.visible());
    h.tick(520);
    assert!(!h.visible());
}

#[test]
fn plain_overlay_requests_follow_macro_mode_rules() {
    let mut h = Harness::new(test_config());
    let r = h.request("overlay", json!({ "action": "show" }), 0).unwrap();
    assert_eq!(r, json!({ "visible": false }), "pinned, but hidden outside macro mode");
    h.macro_mode_on(10);
    assert!(h.visible());
    assert!(h.request("overlay", json!({ "action": "sideways" }), 20).is_err());
}

#[test]
fn output_follows_setting_or_cursor() {
    let mut cfg = test_config();
    cfg.settings.overlay.monitor = Some("DP-1".into());
    let mut h = Harness::new(cfg);
    h.macro_mode_on(0);
    let s = h.core.snapshot();
    assert!(s.overlay.visible);
    assert_eq!(s.overlay.output.as_deref(), Some("DP-1"), "unknown outputs: trust the setting");

    // Follow mouse with KWin: hidden until KWin says where the cursor is.
    let mut h = Harness::new(test_config());
    h.core.set_kwin(true, Vec::new(), h.at(0));
    h.macro_mode_on(0);
    let seq = match h.core.take_effects().iter().find_map(|e| match e {
        Effect::ResolveCursorOutput { seq } => Some(*seq),
        _ => None,
    }) {
        Some(seq) => seq,
        None => panic!("a show must ask for the cursor output"),
    };
    assert!(!h.visible());
    h.core.output_resolved(seq, Some("DP-2".into()), h.at(20));
    let s = h.core.snapshot();
    assert!(s.overlay.visible);
    assert_eq!(s.overlay.output.as_deref(), Some("DP-2"));
}

#[test]
fn disconnected_monitor_falls_back_to_cursor_and_times_out() {
    let mut cfg = test_config();
    cfg.settings.overlay.monitor = Some("HDMI-9".into());
    let mut h = Harness::new(cfg);
    let dp = |name: &str| OutputInfo { name: name.into(), x: 0, y: 0, width: 2560, height: 1440, scale: 1.0, primary: false, model: None };
    h.core.set_kwin(true, vec![dp("DP-1"), dp("DP-2")], h.at(0));
    h.macro_mode_on(0);
    assert!(h.core.take_effects().iter().any(|e| matches!(e, Effect::ResolveCursorOutput { .. })));
    assert!(!h.visible());
    h.tick(OUTPUT_WAIT.as_millis() as u64);
    let s = h.core.snapshot();
    assert!(s.overlay.visible, "KWin too slow: let the compositor choose");
    assert_eq!(s.overlay.output, None);
}

// ---- macros: concurrency, repeat, release ----------------------------------------------------------------

#[test]
fn concurrency_ignore_restart_parallel() {
    let mut cfg = test_config();
    bind(&mut cfg, 0, KeyId::KP1, "sleep(1000)").concurrency = Concurrency::Ignore;
    bind(&mut cfg, 0, KeyId::KP2, "sleep(1000)").concurrency = Concurrency::Restart;
    bind(&mut cfg, 0, KeyId::KP3, "sleep(1000)").concurrency = Concurrency::Parallel;
    let mut h = Harness::new(cfg);
    h.macro_mode_on(0);

    h.tap(KeyId::KP1, 10);
    h.tap(KeyId::KP1, 100);
    assert_eq!(h.spawned().len(), 1, "ignore: a second trigger while running is dropped");

    h.tap(KeyId::KP2, 200);
    let first = *h.spawned().last().unwrap();
    h.tap(KeyId::KP2, 300);
    assert_eq!(h.spawned().len(), 3);
    assert!(h.run_log(first).0, "restart: the running macro is cancelled");
    let second = *h.spawned().last().unwrap();
    assert!(!h.run_log(second).0);

    h.tap(KeyId::KP3, 400);
    h.tap(KeyId::KP3, 500);
    assert_eq!(h.spawned().len(), 5, "parallel: both run");

    // Once the ignored binding's run finishes it can run again.
    let kp1 = h.spawned()[0];
    h.core.run_done(kp1, RunOutcome::Finished, h.at(600));
    h.tap(KeyId::KP1, 700);
    assert_eq!(h.spawned().len(), 6);
}

#[test]
fn repeat_retriggers_only_repeat_bindings() {
    let mut cfg = test_config();
    bind(&mut cfg, 0, KeyId::KP8, "media('volumeUp')").repeat = true;
    bind(&mut cfg, 0, KeyId::KP9, "media('next')").repeat = false;
    let mut h = Harness::new(cfg);
    h.macro_mode_on(0);

    h.key(KeyId::KP8, KeyAction::Press, 10);
    let first = h.spawned()[0];
    h.core.run_done(first, RunOutcome::Finished, h.at(20));
    for (i, ms) in [500u64, 533, 566].into_iter().enumerate() {
        h.key(KeyId::KP8, KeyAction::Repeat, ms);
        let id = h.spawned()[i + 1];
        h.core.run_done(id, RunOutcome::Finished, h.at(ms + 5));
    }
    h.key(KeyId::KP8, KeyAction::Release, 600);
    assert_eq!(h.spawned().len(), 4, "press + three repeats");
    let invs = h.runner.spawned.lock().unwrap();
    assert!(!invs[0].0.repeat && invs[1].0.repeat, "the event says whether it is a repeat");
    drop(invs);

    h.key(KeyId::KP9, KeyAction::Press, 700);
    h.key(KeyId::KP9, KeyAction::Repeat, 1200);
    h.key(KeyId::KP9, KeyAction::Release, 1300);
    assert_eq!(h.spawned().len(), 5, "repeat: false ignores auto-repeat");
}

#[test]
fn release_is_delivered_to_the_press_run_even_after_mode_change() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.key(KeyId::KP5, KeyAction::Press, 10);
    let id = h.spawned()[0];
    assert_eq!(h.run_log(id), (false, 0));
    h.core.engine_event(EngineEvent::ModeChanged { macro_mode: false, source: ModeSource::ToggleKey }, h.at(20));
    h.key(KeyId::KP5, KeyAction::Release, 30);
    assert_eq!(h.run_log(id), (false, 1), "a captured press keeps its release");
}

#[test]
fn run_events_and_failure_toast() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.core.take_effects();
    h.tap(KeyId::KP5, 10);
    let id = h.spawned()[0];
    assert_eq!(h.events(events::RUN), [json!({ "runId": id, "key": "KP5", "status": "started" })]);
    h.core.run_done(id, RunOutcome::Failed("ReferenceError: nope is not defined\n    at line 1:1".into()), h.at(20));
    let effects = h.core.take_effects();
    let toast = effects.iter().find_map(|e| match e {
        Effect::Event(ev) if ev.name == events::TOAST => Some(ev.data.clone()),
        _ => None,
    });
    let toast = toast.expect("toastOnError shows an error toast");
    assert_eq!(toast["kind"], "error");
    assert_eq!(toast["text"], "KP5 macro: ReferenceError: nope is not defined");
    let run = effects.iter().find_map(|e| match e {
        Effect::Event(ev) if ev.name == events::RUN => Some(ev.data.clone()),
        _ => None,
    });
    assert_eq!(run.unwrap()["status"], "failed");
}

#[test]
fn compile_errors_fail_without_spawning() {
    let mut cfg = test_config();
    cfg.profiles[0].keys.get_mut(&KeyId::KP5).unwrap().r#macro =
        Some(Macro::Graph { nodes: Vec::new(), edges: Vec::new() });
    let mut h = Harness::new(cfg);
    h.macro_mode_on(0);
    h.core.take_effects();
    h.tap(KeyId::KP5, 10);
    assert!(h.spawned().is_empty());
    let runs = h.events(events::RUN);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0]["status"], "failed");
}

#[test]
fn cancel_run_and_test_runs() {
    let mut h = Harness::new(test_config());
    let r = h.request("runMacro", json!({ "macro": { "type": "script", "source": "sleep(5000)" } }), 0).unwrap();
    let id = r["runId"].as_u64().unwrap();
    assert_eq!(h.run_log(id), (false, 1), "test runs get their release right away");
    h.request("cancelRun", json!({ "runId": id }), 10).unwrap();
    assert!(h.run_log(id).0);

    let profile_id = h.core.cfg.profiles[0].id.clone();
    let r = h.request("runBinding", json!({ "profileId": profile_id, "key": "KP5" }), 20).unwrap();
    let id = r["runId"].as_u64().unwrap();
    let inv = h.runner.spawned.lock().unwrap().last().unwrap().0.clone();
    assert_eq!((inv.run_id, inv.key, inv.profile_name.as_str()), (id, Some(KeyId::KP5), "A"));
    assert!(h.request("runBinding", json!({ "profileId": profile_id, "key": "KP9" }), 30).is_err());
}

// ---- profiles, capture, learn, simulateKey ---------------------------------------------------------------

#[test]
fn profile_event_precedes_state_with_direction() {
    let mut h = Harness::new(test_config());
    h.request("setProfile", json!({ "delta": -1 }), 0).unwrap();
    assert_eq!(h.core.take_pre_state_events(), [Event::new(events::PROFILE, json!({ "index": 2, "direction": -1 }))]);
    let r = h.request("setProfile", json!({ "index": 1 }), 10).unwrap();
    assert_eq!(r, json!({ "profileIndex": 1 }));
    assert_eq!(h.core.take_pre_state_events()[0].data, json!({ "index": 1, "direction": -1 }));
    let id = h.core.cfg.profiles[2].id.clone();
    h.request("setProfile", json!({ "id": id }), 20).unwrap();
    assert_eq!(h.core.take_pre_state_events()[0].data, json!({ "index": 2, "direction": 1 }));
    assert!(h.request("setProfile", json!({ "index": 7 }), 30).is_err());
    h.request("setProfile", json!({ "index": 2 }), 40).unwrap();
    assert!(h.core.take_pre_state_events().is_empty(), "no change, no event");
}

#[test]
fn capture_set_follows_unbound_keys_setting_and_profile() {
    let h = Harness::new(test_config());
    assert_eq!(h.input.captures.lock().unwrap().last().unwrap().len(), 17, "block = all 17 keys");

    let mut cfg = test_config();
    cfg.settings.unbound_keys = UnboundKeys::Passthrough;
    let mut h = Harness::new(cfg);
    let expected: HashSet<KeyId> = [KeyId::KP5, KeyId::KPEnter, KeyId::KPPlus, KeyId::KPMinus].into();
    assert_eq!(h.input.captures.lock().unwrap().last().unwrap(), &expected, "bound + global keys");
    h.request("setProfile", json!({ "index": 1 }), 0).unwrap();
    let globals_only: HashSet<KeyId> = [KeyId::KPEnter, KeyId::KPPlus, KeyId::KPMinus].into();
    assert_eq!(h.input.captures.lock().unwrap().last().unwrap(), &globals_only);
    h.request("setLearn", json!({ "on": true }), 10).unwrap();
    assert_eq!(h.input.captures.lock().unwrap().last().unwrap().len(), 17, "learn mode captures everything");
}

#[test]
fn learn_mode_reports_instead_of_running_and_expires() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.request("setLearn", json!({ "on": true }), 0).unwrap();
    h.core.take_effects();
    h.tap(KeyId::KP5, 10);
    h.tap(KeyId::KPPlus, 100);
    assert!(h.spawned().is_empty(), "learn mode never executes");
    assert_eq!(h.core.profile_index, 0, "not even globals");
    let learned = h.events(events::LEARN);
    assert_eq!(learned, [json!({ "key": "KP5" }), json!({ "key": "KPPlus" })]);
    assert!(h.core.snapshot().learn);
    h.tick(30_000);
    assert!(!h.core.snapshot().learn, "learn expires after 30 s");

    h.request("setLearn", json!({ "on": true }), 31_000).unwrap();
    h.core.client_gone(2, h.at(31_100));
    assert!(h.core.snapshot().learn, "other clients leaving doesn't matter");
    h.core.client_gone(1, h.at(31_200));
    assert!(!h.core.snapshot().learn, "the client that enabled it leaving ends it");
}

#[test]
fn simulate_key_obeys_macro_mode_and_capture() {
    let mut cfg = test_config();
    cfg.settings.unbound_keys = UnboundKeys::Passthrough;
    let mut h = Harness::new(cfg);
    h.request("simulateKey", json!({ "key": "KP5", "pressed": true }), 0).unwrap();
    h.request("simulateKey", json!({ "key": "KP5", "pressed": false }), 10).unwrap();
    assert!(h.spawned().is_empty(), "ignored outside macro mode");

    h.request("simulateKey", json!({ "key": "Toggle" }), 20).unwrap();
    assert!(h.core.macro_mode);
    assert_eq!(*h.input.modes.lock().unwrap(), [true], "the engine follows API mode changes");
    h.request("simulateKey", json!({ "key": "kp7", "pressed": true }), 30).unwrap();
    assert!(h.core.held.is_empty(), "uncaptured keys pass through");
    h.request("simulateKey", json!({ "key": "KP5", "pressed": true }), 40).unwrap();
    h.request("simulateKey", json!({ "key": "KP5", "pressed": false }), 50).unwrap();
    assert_eq!(h.spawned().len(), 1);
    assert!(h.request("simulateKey", json!({ "key": "KP99" }), 60).is_err());
}

#[test]
fn empty_keys_emit_empty_key_events() {
    let mut h = Harness::new(test_config());
    h.macro_mode_on(0);
    h.core.take_effects();
    h.tap(KeyId::KP9, 10);
    assert_eq!(h.events(events::KEY)[0], json!({ "key": "KP9", "pressed": true, "kind": "empty" }));
}

// ---- host calls, vars, state -----------------------------------------------------------------------------

#[test]
fn vars_toggle_is_atomic_and_persisted() {
    let mut h = Harness::new(test_config());
    assert_eq!(h.core.host_call("vars.toggle", json!({ "name": "gameMode" }), h.at(0)), Ok(json!(true)));
    assert_eq!(h.core.host_call("vars.toggle", json!({ "name": "gameMode" }), h.at(1)), Ok(json!(false)));
    h.core.host_call("vars.set", json!({ "name": "n", "value": { "a": [1, 2] } }), h.at(2)).unwrap();
    assert_eq!(h.core.host_call("vars.get", json!({ "name": "n" }), h.at(3)), Ok(json!({ "a": [1, 2] })));
    assert_eq!(h.core.host_call("vars.get", json!({ "name": "missing" }), h.at(4)), Ok(Value::Null));
    assert_eq!(h.core.persisted().vars.get("gameMode"), Some(&json!(false)));
    assert_eq!(h.request("getVars", Value::Null, 5).unwrap()["n"], json!({ "a": [1, 2] }));
}

#[test]
fn host_profile_and_mode_calls() {
    let mut h = Harness::new(test_config());
    let now = h.at(0);
    h.core.host_call("profile.set", json!({ "target": "C" }), now).unwrap();
    assert_eq!(h.core.host_call("profile.current", json!({}), now).unwrap()["name"], "C");
    h.core.host_call("profile.set", json!({ "target": "next" }), now).unwrap();
    assert_eq!(h.core.profile_index, 0);
    h.core.host_call("profile.set", json!({ "target": 1 }), now).unwrap();
    assert_eq!(h.core.profile_index, 1);
    assert!(h.core.host_call("profile.set", json!({ "target": "Nope" }), now).is_err());
    assert_eq!(h.core.host_call("macroMode.toggle", json!({}), now), Ok(json!(true)));
    assert_eq!(h.core.host_call("macroMode.get", json!({}), now), Ok(json!(true)));
    h.core.host_call("toast", json!({ "text": "hi", "kind": "success" }), now).unwrap();
    let toast = h.events(events::TOAST);
    assert_eq!((toast[0]["text"].as_str(), toast[0]["kind"].as_str()), (Some("hi"), Some("success")));
    assert!(h.core.host_call("no.such", json!({}), now).is_err());
}

#[test]
fn restore_from_persisted_state_only_with_remember_state() {
    let cfg = test_config();
    let persisted = PersistedState {
        macro_mode: true,
        profile_id: cfg.profiles[2].id.clone(),
        pinned: true,
        vars: BTreeMap::from([("x".into(), json!(1))]),
    };
    let initial = Initial::new(&cfg, Some(&persisted));
    assert_eq!((initial.macro_mode, initial.profile_index, initial.pinned), (true, 2, true));

    let mut forget = cfg.clone();
    forget.settings.remember_state = false;
    forget.settings.start_in_macro_mode = true;
    let initial = Initial::new(&forget, Some(&persisted));
    assert_eq!((initial.macro_mode, initial.profile_index, initial.pinned), (true, 0, false));
    assert_eq!(initial.vars.get("x"), Some(&json!(1)), "vars always persist");

    let h = Harness::with_initial(cfg, |i| i.pinned = true);
    assert!(h.core.persisted().pinned);
}

#[test]
fn config_reload_keeps_profile_by_id_and_reports_errors() {
    let mut h = Harness::new(test_config());
    h.request("setProfile", json!({ "index": 2 }), 0).unwrap();
    let mut cfg = h.core.cfg.clone();
    cfg.profiles.swap(0, 2);
    cfg.library = "function helper() {}".into();
    cfg.settings.toggle_key = "Pause".into();
    h.core.apply_config(cfg, h.at(10));
    assert_eq!(h.core.profile_index, 0, "the same profile stays selected after reordering");
    assert_eq!(*h.runner.library.lock().unwrap(), "function helper() {}");
    assert_eq!(*h.input.reconfigured.lock().unwrap(), 1, "toggle key change reconfigures the engine");

    h.core.take_effects();
    h.core.config_failed("invalid config.json: oops".into(), h.at(20));
    assert_eq!(h.events(events::CONFIG_ERROR), [json!({ "message": "invalid config.json: oops" })]);
    assert_eq!(h.core.snapshot().config_error.as_deref(), Some("invalid config.json: oops"));
    let cfg = h.core.cfg.clone();
    h.core.apply_config(cfg, h.at(30));
    assert_eq!(h.core.snapshot().config_error, None, "a good config clears the error");
}

#[test]
fn snapshot_renders_current_profile_and_globals() {
    let mut cfg = test_config();
    cfg.profiles[1].color = Some("#ff7a59".into());
    cfg.settings.globals.insert(KeyId::KPDot, GlobalBinding { tap: GlobalAction::ToggleMacroMode, hold: None });
    let mut h = Harness::new(cfg);
    let s = h.core.snapshot();
    assert_eq!(s.profiles.len(), 3);
    assert_eq!(s.profile.keys.len(), 17);
    assert_eq!(s.profile.keys[&KeyId::KP5].kind, RenderKeyKind::Macro);
    assert_eq!(s.profile.keys[&KeyId::KPDot].global, Some(GlobalAction::ToggleMacroMode));
    assert_eq!(s.overlay.accent, "#8b7cf6");
    h.request("setProfile", json!({ "index": 1 }), 0).unwrap();
    assert_eq!(h.core.snapshot().overlay.accent, "#ff7a59", "profile colour overrides the accent");

    h.macro_mode_on(10);
    h.tap(KeyId::KPDot, 20);
    assert!(!h.core.macro_mode, "a toggleMacroMode global leaves macro mode");
}
