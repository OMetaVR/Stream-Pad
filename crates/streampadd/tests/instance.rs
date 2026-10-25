//! Single-instance locking. A separate test binary: tests that fork elsewhere in the suite would briefly
//! share the lock's file description and make "released lock" assertions racy.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use streampadd::instance::{Instance, InstanceError};

#[test]
fn second_instance_is_refused_and_socket_is_private() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("run/daemon.sock");
    let first = Instance::acquire(&socket).unwrap();
    let mode = fs::metadata(&socket).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    assert!(matches!(Instance::acquire(&socket), Err(InstanceError::AlreadyRunning(_))));
    drop(first);
    assert!(!socket.exists(), "the socket is removed on exit");
    if let Err(e) = Instance::acquire(&socket) {
        panic!("a released lock must not block a restart: {e}");
    }
}

#[test]
fn stale_socket_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("daemon.sock");
    drop(UnixListener::bind(&socket).unwrap());
    assert!(socket.exists());
    assert!(Instance::acquire(&socket).is_ok());
}
