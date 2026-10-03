use crate::host::*;
use execution_app::*;
#[test]
fn only_current_envelope_calls_the_handler() {
    struct Spy(usize);
    impl Handler for Spy {
        fn handle(&mut self, _: &Peer, _: Request) -> Reply {
            self.0 += 1;
            Reply::Unavailable
        }
        fn tick(&mut self) -> Result<(), Error> {
            Ok(())
        }
        fn stop(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }
    let mut spy = Spy(0);
    let peer = Peer {
        pid: 1,
        uid: Some(1),
        session: 1,
        #[cfg(windows)]
        native: 0,
    };
    for version in [1, 2, 3, 4, 5, 6, 7] {
        let bytes = serde_json::to_vec(
            &serde_json::json!({"version":version,"request":{"method":"status","request":"r"}}),
        )
        .unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&dispatch(&mut spy, &peer, &bytes))
                .unwrap(),
            serde_json::json!({"version":8,"reply":{"kind":"rejected"}})
        );
        assert_eq!(spy.0, 0);
    }
    assert_eq!(
        dispatch(
            &mut spy,
            &peer,
            br#"{"version":8,"request":{"method":"status","request":"r"}}"#
        ),
        br#"{"version":8,"reply":{"kind":"unavailable"}}"#
    );
    assert_eq!(spy.0, 1);
}
