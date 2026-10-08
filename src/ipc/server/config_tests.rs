use super::*;
use crate::tests::Fixture;
use niri_ipc::ConfigChange;
use std::time::{Duration, Instant};

fn request(fixture: &mut Fixture, request: Request) -> Reply {
    let state = fixture.niri_state();
    let server = state.niri.ipc_server.as_ref().unwrap();
    let ctx = ClientCtx {
        event_loop: state.niri.event_loop.clone(),
        scheduler: state.niri.scheduler.clone(),
        ipc_outputs: state.backend.ipc_outputs(),
        event_streams: server.event_streams.clone(),
        event_stream_state: server.event_stream_state.clone(),
    };
    let (sender, receiver) = async_channel::bounded(1);
    state
        .niri
        .scheduler
        .schedule(async move {
            let reply = process(&ctx, request).await;
            sender.send(reply).await.unwrap();
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Ok(reply) = receiver.try_recv() {
            return reply;
        }
        let remaining = deadline.checked_duration_since(Instant::now()).unwrap();
        let server = &mut fixture.state.server;
        server
            .event_loop
            .dispatch(remaining, &mut server.state)
            .unwrap();
        server.state.refresh_and_flush_clients();
    }
}

fn map(fixture: &mut Fixture) -> Vec<u64> {
    let client = fixture.add_client();
    let window = fixture.client(client).create_window();
    window.xdg_toplevel.set_app_id("p39-owned-window".into());
    let surface = window.surface.clone();
    window.commit();
    fixture.roundtrip(client);
    let window = fixture.client(client).window(&surface);
    window.attach_new_buffer();
    window.ack_last_and_commit();
    fixture.double_roundtrip(client);
    fixture.niri_complete_animations();
    fixture.niri_state().ipc_refresh_layout();
    windows(fixture)
}

fn windows(fixture: &mut Fixture) -> Vec<u64> {
    let mut windows: Vec<_> = fixture
        .niri()
        .ipc_server
        .as_ref()
        .unwrap()
        .event_stream_state
        .borrow()
        .windows
        .windows
        .keys()
        .copied()
        .collect();
    windows.sort_unstable();
    windows
}

#[test]
fn unchanged_config_and_retained_display_resources_are_classified_by_niri() {
    let mut fixture = Fixture::with_config(niri_config::Config::default());
    let mut same = niri_config::Config::default();
    assert_eq!(
        fixture.niri_state().config_change(&mut same),
        ConfigChange::Unchanged
    );
    let mut layout = niri_config::Config::parse_mem("layout { gaps 17; }").unwrap();
    assert_eq!(
        fixture.niri_state().config_change(&mut layout),
        ConfigChange::Reload
    );
    let mut device =
        niri_config::Config::parse_mem("debug { render-drm-device \"/dev/dri/renderD129\"; }")
            .unwrap();
    assert!(
        matches!(fixture.niri_state().config_change(&mut device), ConfigChange::Restart { fields } if fields == ["debug.render-drm-device"])
    );
    assert!(!fixture.niri_state().reload_config(Ok(device)));
    assert!(fixture
        .niri()
        .config
        .borrow()
        .debug
        .render_drm_device
        .is_none());
}

#[test]
fn inspection_does_not_apply_and_correlated_reload_preserves_mapped_windows() {
    let root = std::env::temp_dir().join(format!("niri-config-ipc-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("config.kdl");
    std::fs::write(&path, "layout { gaps 17; }").unwrap();
    let mut fixture = Fixture::with_config(niri_config::Config::default());
    fixture.add_output(1, (1920, 1080));
    crate::utils::watcher::setup(
        fixture.niri_state(),
        &niri_config::ConfigPath::Explicit(path.clone()),
        Vec::new(),
    );
    let before = map(&mut fixture);
    assert!(!before.is_empty());
    let reply = request(
        &mut fixture,
        Request::Config {
            path: path.to_str().unwrap().into(),
            correlation: "inspect-1".into(),
            apply: false,
        },
    );
    assert!(matches!(reply, Ok(Response::Config(ConfigChange::Reload))));
    assert_eq!(
        fixture
            .niri()
            .ipc_server
            .as_ref()
            .unwrap()
            .event_stream_state
            .borrow()
            .config
            .correlation,
        None
    );
    for correlation in ["load-1", "load-2"] {
        let reply = request(
            &mut fixture,
            Request::Config {
                path: path.to_str().unwrap().into(),
                correlation: correlation.into(),
                apply: true,
            },
        );
        if correlation == "load-1" {
            assert!(matches!(reply, Ok(Response::Config(ConfigChange::Reload))));
        } else {
            assert!(matches!(
                reply,
                Ok(Response::Config(ConfigChange::Unchanged))
            ));
        }
        let server = fixture.niri().ipc_server.as_ref().unwrap();
        let state = server.event_stream_state.borrow();
        assert!(!state.config.failed);
        assert_eq!(state.config.correlation.as_deref(), Some(correlation));
        assert!(
            matches!(state.config.replicate().as_slice(), [Event::ConfigLoaded { failed: false, correlation: Some(observed) }] if observed == correlation)
        );
        drop(state);
        fixture.niri_state().ipc_refresh_layout();
        assert_eq!(windows(&mut fixture), before);
    }
    std::fs::write(&path, "layout { gaps \"invalid\"; }").unwrap();
    assert!(request(
        &mut fixture,
        Request::Config {
            path: path.to_str().unwrap().into(),
            correlation: "invalid-load".into(),
            apply: true
        }
    )
    .is_err());
    assert_eq!(windows(&mut fixture), before);
    std::fs::remove_dir_all(root).unwrap();
}
