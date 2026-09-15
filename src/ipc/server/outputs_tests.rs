use super::*;
use crate::tests::fixture::Fixture;

#[test]
fn output_changes_replicate_before_the_snapshot_boundary_without_duplicates() {
    let mut fixture = Fixture::new();
    let (events, received) = async_channel::bounded(EVENT_STREAM_BUFFER_SIZE);
    let (disconnect, _disconnected) = async_channel::bounded(1);
    fixture
        .niri()
        .ipc_server
        .as_ref()
        .unwrap()
        .event_streams
        .borrow_mut()
        .push(EventStreamSender { events, disconnect });
    fixture.add_output(1, (1920, 1080));
    fixture.niri_state().refresh_ipc_outputs();
    fixture.niri_state().ipc_refresh_outputs();
    let Event::OutputsChanged { outputs } = received.try_recv().unwrap() else {
        panic!("expected output snapshot");
    };
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].name, "headless-1");
    assert!(received.try_recv().is_err());

    let backend = fixture.niri_state().backend.ipc_outputs();
    backend
        .lock()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()
        .logical
        .as_mut()
        .unwrap()
        .scale = 1.5;
    fixture.niri_state().ipc_refresh_outputs();
    let Event::OutputsChanged { outputs } = received.try_recv().unwrap() else {
        panic!("expected scale change");
    };
    assert_eq!(outputs[0].logical.unwrap().scale, 1.5);

    let burst = fixture
        .niri()
        .ipc_server
        .as_ref()
        .unwrap()
        .event_stream_state
        .borrow()
        .replicate();
    assert!(matches!(burst.last(), Some(Event::CastsChanged { .. })));
    let mut restored = EventStreamState::default();
    for event in burst {
        assert!(restored.apply(event).is_none());
    }
    assert_eq!(
        restored.outputs.outputs["headless-1"]
            .logical
            .unwrap()
            .scale,
        1.5
    );

    backend.lock().unwrap().clear();
    fixture.niri_state().ipc_refresh_outputs();
    let event = received.try_recv().unwrap();
    assert!(matches!(&event, Event::OutputsChanged { outputs } if outputs.is_empty()));
    restored.apply(event);
    assert!(restored.outputs.outputs.is_empty());
    fixture.niri_state().ipc_refresh_outputs();
    assert!(received.try_recv().is_err());
}
