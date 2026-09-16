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
