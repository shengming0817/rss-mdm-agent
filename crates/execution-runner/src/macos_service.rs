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
        while !stop.load(Ordering::Acquire) {
            if let Ok(mut h) = HANDLER.get().unwrap().lock() {
                h.tick();
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if let Ok(mut h) = HANDLER.get().unwrap().lock() {
            h.stop();
        }
    });
    let result = unsafe { rss_execution_listen() };
    stop.store(true, Ordering::Release);
    let _ = worker.join();
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
    std::panic::catch_unwind(|| {
        let Some(handler) = HANDLER.get() else {
            return -1;
        };
        let Ok(mut h) = handler.lock() else { return -1 };
        let peer = Peer {
            pid,
            uid,
            session,
            native: connection as usize,
        };
        let reply = host::dispatch(h.as_mut(), &peer, unsafe {
            std::slice::from_raw_parts(data, size)
        });
        if reply.len() > capacity {
            return -1;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(reply.as_ptr(), output, reply.len());
        }
        reply.len() as isize
    })
    .unwrap_or(-1)
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
