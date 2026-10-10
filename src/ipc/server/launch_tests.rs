use super::*;
use crate::tests::Fixture;

fn map(fixture: &mut Fixture, app_id: &str) {
    let client = fixture.add_client();
    let window = fixture.client(client).create_window();
    window.xdg_toplevel.set_app_id(app_id.into());
    let surface = window.surface.clone();
    window.commit();
    fixture.roundtrip(client);
    let window = fixture.client(client).window(&surface);
    window.attach_new_buffer();
    window.ack_last_and_commit();
    fixture.double_roundtrip(client);
    fixture.niri_complete_animations();
    fixture.niri_state().ipc_refresh_layout();
}

#[test]
fn windowed_fullscreen_modes_are_visible_in_the_first_mapping_event() {
    for maximized in [false, true] {
        let config = niri_config::Config::parse_mem(&format!(
            r#"
            workspace "work" {{ open-on-output "headless-1"; }}
            workspace "other" {{ open-on-output "headless-1"; }}
            window-rule {{
                match app-id="^owner$"
                open-on-workspace "other"
                open-focused true
            }}
            window-rule {{
                match app-id="^restored$"
                open-on-workspace "work"
                open-focused false
                default-column-width {{ proportion 0.5; }}
                open-maximized-to-edges {maximized}
                open-fullscreen false
                open-windowed-fullscreen true
                open-tiled-insertion {{}}
            }}
        "#
        ))
        .unwrap();
        let mut fixture = Fixture::with_config(config);
        fixture.add_output(1, (1920, 1080));
        map(&mut fixture, "owner");
        let owner = windows(&mut fixture).remove(0);
        let (events, received) = async_channel::bounded(EVENT_STREAM_BUFFER_SIZE);
        let (disconnect, _) = async_channel::bounded(1);
        fixture
            .niri()
            .ipc_server
            .as_ref()
            .unwrap()
            .event_streams
            .borrow_mut()
            .push(EventStreamSender { events, disconnect });
        map(&mut fixture, "restored");
        let mut first_mapping = None;
        while let Ok(event) = received.try_recv() {
            if let Event::WindowOpenedOrChanged { window } = event {
                if window.app_id.as_deref() == Some("restored") && first_mapping.is_none() {
                    first_mapping = Some(window);
                }
            }
        }
        let restored = first_mapping.unwrap();
        assert!(restored.is_windowed_fullscreen);
        assert_eq!(restored.is_maximized, maximized);
        assert!(!restored.is_fullscreen);
        assert!(!restored.is_focused);
        assert_eq!(restored.layout.pos_in_scrolling_layout, Some((1, 1)));
        assert!(windows(&mut fixture)
            .iter()
            .any(|window| window.id == owner.id && window.is_focused));
    }
}

fn windows(fixture: &mut Fixture) -> Vec<niri_ipc::Window> {
    fixture
        .niri()
        .ipc_server
        .as_ref()
        .unwrap()
        .event_stream_state
        .borrow()
        .windows
        .windows
        .values()
        .cloned()
        .collect()
}

#[test]
fn a_configured_cohort_maps_in_every_order_from_its_incarnation_references() {
    for order in [
        [1, 2, 3],
        [1, 3, 2],
        [2, 1, 3],
        [2, 3, 1],
        [3, 1, 2],
        [3, 2, 1],
    ] {
        let config = niri_config::Config::parse_mem(
            r#"
            workspace "work" { open-on-output "headless-1"; }
            workspace "other" { open-on-output "headless-1"; }
            window-rule {
                match app-id="^owner$"
                open-on-workspace "other"
                open-focused true
            }
            window-rule {
                match app-id="^managed[.]1[.]epoch$"
                open-on-workspace "work"
                open-focused false
                open-tiled-insertion {
                    right app-id="^managed[.]2[.]epoch$"
                    right app-id="^managed[.]3[.]epoch$"
                }
            }
            window-rule {
                match app-id="^managed[.]2[.]epoch$"
                open-on-workspace "work"
                open-focused false
                open-tiled-insertion {
                    left app-id="^managed[.]1[.]epoch$"
                    right app-id="^managed[.]3[.]epoch$"
                }
            }
            window-rule {
                match app-id="^managed[.]3[.]epoch$"
                open-on-workspace "work"
                open-focused false
                open-tiled-insertion {
                    left app-id="^managed[.]2[.]epoch$"
                    left app-id="^managed[.]1[.]epoch$"
                }
            }
            "#,
        )
        .unwrap();
        let mut fixture = Fixture::with_config(config);
        fixture.add_output(1, (1920, 1080));
        map(&mut fixture, "owner");
        let owner = windows(&mut fixture).remove(0);
        let (events, received) = async_channel::bounded(EVENT_STREAM_BUFFER_SIZE);
        let (disconnect, _) = async_channel::bounded(1);
        fixture
            .niri()
            .ipc_server
            .as_ref()
            .unwrap()
            .event_streams
            .borrow_mut()
            .push(EventStreamSender { events, disconnect });
        let mut configured = Vec::new();
        for id in 1..=3 {
            let client = fixture.add_client();
            let window = fixture.client(client).create_window();
            window
                .xdg_toplevel
                .set_app_id(format!("managed.{id}.epoch"));
            let surface = window.surface.clone();
            window.commit();
            fixture.roundtrip(client);
            configured.push((client, surface));
        }
        assert_eq!(windows(&mut fixture).len(), 1);
        for id in order {
            let (client, surface) = &configured[id - 1];
            let window = fixture.client(*client).window(surface);
            window.attach_new_buffer();
            window.ack_last_and_commit();
            fixture.double_roundtrip(*client);
            fixture.niri_complete_animations();
            fixture.niri_state().ipc_refresh_layout();
            assert!(windows(&mut fixture)
                .iter()
                .any(|window| window.id == owner.id && window.is_focused));
        }
        let mut restored: Vec<_> = windows(&mut fixture)
            .into_iter()
            .filter(|window| window.id != owner.id)
            .collect();
        restored.sort_by_key(|window| window.layout.pos_in_scrolling_layout);
        assert_eq!(
            restored
                .iter()
                .map(|window| window.app_id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["managed.1.epoch", "managed.2.epoch", "managed.3.epoch"]
        );
        assert!(restored
            .iter()
            .all(|window| window.workspace_id == restored[0].workspace_id && !window.is_focused));
        assert_ne!(restored[0].workspace_id, owner.workspace_id);
        while let Ok(event) = received.try_recv() {
            if let Event::WindowFocusChanged { id } = event {
                assert_eq!(id, Some(owner.id));
            }
        }
    }
}

#[test]
fn a_deferred_member_in_a_later_cohort_precedes_an_earlier_slow_mapping() {
    for stacked in [false, true] {
        let (before, after) = if stacked {
            ("above", "below")
        } else {
            ("left", "right")
        };
        let config = niri_config::Config::parse_mem(&format!(
            r#"
            workspace "work" {{ open-on-output "headless-1"; }}
            workspace "other" {{ open-on-output "headless-1"; }}
            window-rule {{ match app-id="^owner$"; open-on-workspace "other"; open-focused true; }}
            window-rule {{
                match app-id="^managed[.]a[.]epoch1$"
                open-on-workspace "work"
                open-focused false
                open-tiled-insertion {{
                    {after} app-id="^managed[.]b[.].*$"
                    {after} app-id="^managed[.]c[.]epoch1$"
                }}
            }}
            window-rule {{
                match app-id="^managed[.]b[.]epoch2$"
                open-on-workspace "work"
                open-focused false
                open-tiled-insertion {{
                    {before} app-id="^managed[.]a[.]epoch1$"
                    {after} app-id="^managed[.]c[.]epoch1$"
                }}
            }}
            window-rule {{
                match app-id="^managed[.]c[.]epoch1$"
                open-on-workspace "work"
                open-focused false
                open-tiled-insertion {{
                    {before} app-id="^managed[.]b[.].*$"
                    {before} app-id="^managed[.]a[.]epoch1$"
                }}
            }}
        "#
        ))
        .unwrap();
        let mut fixture = Fixture::with_config(config);
        fixture.add_output(1, (1920, 1080));
        map(&mut fixture, "owner");
        let owner = windows(&mut fixture).remove(0);
        let client = fixture.add_client();
        let window = fixture.client(client).create_window();
        window.xdg_toplevel.set_app_id("managed.a.epoch1".into());
        let surface = window.surface.clone();
        window.commit();
        fixture.roundtrip(client);
        map(&mut fixture, "managed.c.epoch1");
        map(&mut fixture, "managed.b.epoch2");
        let window = fixture.client(client).window(&surface);
        window.attach_new_buffer();
        window.ack_last_and_commit();
        fixture.double_roundtrip(client);
        fixture.niri_complete_animations();
        fixture.niri_state().ipc_refresh_layout();
        let mut restored: Vec<_> = windows(&mut fixture)
            .into_iter()
            .filter(|window| window.id != owner.id)
            .collect();
        restored.sort_by_key(|window| window.layout.pos_in_scrolling_layout);
        assert_eq!(
            restored
                .iter()
                .map(|window| window.app_id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["managed.a.epoch1", "managed.b.epoch2", "managed.c.epoch1"]
        );
        assert_eq!(
            restored
                .iter()
                .map(|window| window.layout.pos_in_scrolling_layout.unwrap())
                .collect::<Vec<_>>(),
            if stacked {
                vec![(1, 1), (1, 2), (1, 3)]
            } else {
                vec![(1, 1), (2, 1), (3, 1)]
            }
        );
        assert!(windows(&mut fixture)
            .iter()
            .any(|window| window.id == owner.id && window.is_focused));
    }
}

#[test]
fn routed_launch_beside_an_unfocused_anchor_preserves_the_owners_workspace() {
    let config = niri_config::Config::parse_mem(
        r#"
        workspace "work" { open-on-output "headless-1"; }
        workspace "other" { open-on-output "headless-1"; }
        window-rule {
            match app-id="^anchor$"
            open-on-workspace "work"
            open-focused true
        }
        window-rule {
            match app-id="^owner$"
            open-on-workspace "other"
            open-focused true
        }
        window-rule {
            match app-id="^mesh[.]fixture[.]niri[.]work[.]"
            open-on-workspace "work"
            open-focused false
            default-column-width { proportion 0.5; }
        }
    "#,
    )
    .unwrap();
    let mut fixture = Fixture::with_config(config);
    fixture.add_output(1, (1920, 1080));
    map(&mut fixture, "anchor");
    map(&mut fixture, "owner");
    let before = windows(&mut fixture);
    let anchor = before
        .iter()
        .find(|window| window.app_id.as_deref() == Some("anchor"))
        .unwrap();
    let owner = before
        .iter()
        .find(|window| window.app_id.as_deref() == Some("owner"))
        .unwrap();
    assert!(!anchor.is_focused);
    assert!(owner.is_focused);
    assert_ne!(anchor.workspace_id, owner.workspace_id);
    let (events, received) = async_channel::bounded(EVENT_STREAM_BUFFER_SIZE);
    let (disconnect, _) = async_channel::bounded(1);
    fixture
        .niri()
        .ipc_server
        .as_ref()
        .unwrap()
        .event_streams
        .borrow_mut()
        .push(EventStreamSender { events, disconnect });
    map(&mut fixture, "mesh.fixture.niri.work.created");
    let after = windows(&mut fixture);
    let created = after
        .iter()
        .find(|window| window.app_id.as_deref() == Some("mesh.fixture.niri.work.created"))
        .unwrap();
    assert_eq!(created.workspace_id, anchor.workspace_id);
    assert_eq!(
        created.layout.pos_in_scrolling_layout,
        Some((anchor.layout.pos_in_scrolling_layout.unwrap().0 + 1, 1))
    );
    assert!(!created.is_focused);
    assert!(
        after
            .iter()
            .find(|window| window.id == owner.id)
            .unwrap()
            .is_focused
    );
    let state = fixture
        .niri()
        .ipc_server
        .as_ref()
        .unwrap()
        .event_stream_state
        .borrow();
    assert!(state.workspaces.workspaces[&owner.workspace_id.unwrap()].is_focused);
    assert!(!state.workspaces.workspaces[&anchor.workspace_id.unwrap()].is_focused);
    while let Ok(event) = received.try_recv() {
        match event {
            Event::WindowFocusChanged { id } => assert_eq!(id, Some(owner.id)),
            Event::WindowOpenedOrChanged { window } if window.id == created.id => {
                assert!(!window.is_focused)
            }
            _ => {}
        }
    }
}
