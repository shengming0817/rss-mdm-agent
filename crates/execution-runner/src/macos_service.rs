use crate::host::{self, Handler, Peer};
use execution_app::Error;
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::Duration,
};
static HANDLER: OnceLock<Mutex<Box<dyn Handler>>> = OnceLock::new();
static STOP: OnceLock<&'static AtomicBool> = OnceLock::new();
extern "C" {
    fn rss_execution_listen() -> i32;
    fn rss_execution_query(
        request: *const u8,
        length: usize,
        system: i32,
        output: *mut u8,
        capacity: *mut usize,
    ) -> i32;
}
pub(crate) fn run(handler: Box<dyn Handler>, stop: &'static AtomicBool) -> Result<(), Error> {
    HANDLER
        .set(Mutex::new(handler))
        .map_err(|_| Error::Conflict)?;
    STOP.set(stop).map_err(|_| Error::Conflict)?;
    let worker = std::thread::spawn(move || {
        let mut previous = None;
        while !stop.load(Ordering::Acquire) {
            let mut h = HANDLER.get().unwrap().lock().map_err(|_| {
                stop.store(true, Ordering::Release);
                eprintln!("execution handler unavailable");
                Error::Unavailable
            })?;
            {
                match h.tick() {
                    Ok(()) => previous = None,
                    Err(error) => {
                        if previous != Some(error) {
                            eprintln!("execution reconcile: {error}");
                            previous = Some(error);
                        }
                    }
                }
            }
            drop(h);
            std::thread::sleep(Duration::from_millis(100));
        }
        HANDLER
            .get()
            .unwrap()
            .lock()
            .map_err(|_| Error::Unavailable)?
            .stop()?;
        Ok::<(), Error>(())
    });
    let result = unsafe { rss_execution_listen() };
    stop.store(true, Ordering::Release);
    worker.join().map_err(|_| Error::Unavailable)??;
    if result == 0 {
        Ok(())
    } else {
        Err(Error::Unavailable)
    }
}
#[no_mangle]
extern "C" fn rss_execution_stopping() -> i32 {
    i32::from(STOP.get().is_none_or(|s| s.load(Ordering::Acquire)))
}
#[no_mangle]
unsafe extern "C" fn rss_execution_call(
    connection: *mut c_void,
    pid: u32,
    uid: u32,
    session: u32,
    data: *const u8,
    size: usize,
    output: *mut u8,
    capacity: usize,
) -> isize {
    // The native stack strongly retains the NSXPCConnection for the entire synchronous call.
    if connection.is_null()
        || data.is_null()
        || output.is_null()
        || size > 65536
        || rss_execution_stopping() != 0
    {
        return -1;
    }
    let Some(handler) = HANDLER.get() else {
        return -1;
    };
    let Some(stop) = STOP.get() else { return -1 };
    let peer = Peer {
        pid,
        uid: Some(uid),
        session,
        native: connection as usize,
    };
    let reply = handle_call(handler, stop, &peer, unsafe {
        std::slice::from_raw_parts(data, size)
    });
    match reply {
        Ok(reply) if reply.len() <= capacity => {
            unsafe {
                std::ptr::copy_nonoverlapping(reply.as_ptr(), output, reply.len());
            }
            reply.len() as isize
        }
        _ => -1,
    }
}
/// Bounded raw transport for local mechanism verification; replies are not authenticated results.
pub fn query(bytes: &[u8], system: bool) -> Result<Vec<u8>, Error> {
    if bytes.len() > 65536 {
        return Err(Error::Capacity);
    }
    let mut output = vec![0; 65536];
    let mut size = output.len();
    if unsafe {
        rss_execution_query(
            bytes.as_ptr(),
            bytes.len(),
            i32::from(system),
            output.as_mut_ptr(),
            &mut size,
        )
    } != 0
        || size > output.len()
    {
        return Err(Error::Unavailable);
    }
    output.truncate(size);
    Ok(output)
}

fn handle_call(
    handler: &Mutex<Box<dyn Handler>>,
    stop: &AtomicBool,
    peer: &Peer,
    bytes: &[u8],
) -> Result<Vec<u8>, Error> {
    if stop.load(Ordering::Acquire) {
        return Err(Error::Unavailable);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut h = handler.lock().map_err(|_| Error::Unavailable)?;
        Ok(host::dispatch(h.as_mut(), peer, bytes))
    }))
    .unwrap_or(Err(Error::Unavailable));
    if result.is_err() && !stop.swap(true, Ordering::AcqRel) {
        eprintln!("execution handler unavailable");
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Panicking;
    impl Handler for Panicking {
        fn handle(&mut self, _: &Peer, _: host::Request) -> host::Reply {
            panic!("injected handler fault")
        }
        fn tick(&mut self) -> Result<(), Error> {
            Ok(())
        }
        fn stop(&mut self) -> Result<(), Error> {
            Ok(())
        }
    }
    #[test]
    fn poisoned_handler_stops_the_service_instead_of_silently_skipping_work() {
        let handler: Mutex<Box<dyn Handler>> = Mutex::new(Box::new(Panicking));
        let stop = AtomicBool::new(false);
        let peer = Peer {
            pid: 1,
            uid: Some(1),
            session: 1,
            native: 0,
        };
        assert!(handle_call(
            &handler,
            &stop,
            &peer,
            br#"{"version":2,"request":{"method":"status","request":"r"}}"#
        )
        .is_err());
        assert!(stop.load(Ordering::Acquire));
        assert!(handler.is_poisoned());
    }
}
