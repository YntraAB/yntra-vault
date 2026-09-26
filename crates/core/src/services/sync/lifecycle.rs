//! Operation ownership and interruptible socket I/O for desktop P2P workers.
use std::net::TcpStream;
use std::io::{self, Read, Write};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};

#[derive(Default)]
pub struct OperationSlot(Mutex<Option<Arc<AtomicBool>>>);

impl OperationSlot {
    pub fn begin(self: &Arc<Self>) -> Result<OperationLease, String> {
        let mut active = self.0.lock().map_err(|e| e.to_string())?;
        if active.is_some() { return Err("A previous connection is still closing. Please retry shortly.".into()); }
        let cancel = Arc::new(AtomicBool::new(false));
        *active = Some(cancel.clone());
        Ok(OperationLease { slot: self.clone(), cancel })
    }

    pub fn cancel(&self) {
        if let Ok(active) = self.0.lock() {
            if let Some(cancel) = active.as_ref() { cancel.store(true, Ordering::Release); }
        }
    }

    pub fn is_idle(&self) -> bool { self.0.lock().map(|v| v.is_none()).unwrap_or(false) }
}

pub struct OperationLease { slot: Arc<OperationSlot>, pub cancel: Arc<AtomicBool> }
impl Drop for OperationLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.slot.0.lock() {
            if active.as_ref().is_some_and(|flag| Arc::ptr_eq(flag, &self.cancel)) { *active = None; }
        }
    }
}

/// Short socket waits preserve partially transferred frames while allowing
/// cancellation on Windows, where shutdown of a cloned socket need not wake recv.
pub struct CancellableStream { stream: TcpStream, cancel: Option<Arc<AtomicBool>> }
impl CancellableStream {
    pub fn new(stream: TcpStream, cancel: Option<Arc<AtomicBool>>) -> io::Result<Self> {
        stream.set_read_timeout(Some(Duration::from_millis(200)))?;
        stream.set_write_timeout(Some(Duration::from_millis(200)))?;
        Ok(Self { stream, cancel })
    }
    fn io<T>(&mut self, mut operation: impl FnMut(&mut TcpStream) -> io::Result<T>) -> io::Result<T> {
        let start = Instant::now();
        loop {
            if self.cancel.as_ref().is_some_and(|c| c.load(Ordering::Acquire)) {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "Connection cancelled"));
            }
            match operation(&mut self.stream) {
                Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => {
                    if start.elapsed() >= Duration::from_secs(30) { return Err(io::Error::new(io::ErrorKind::TimedOut, "Peer did not respond within 30 seconds")); }
                }
                result => return result,
            }
        }
    }
}
impl Read for CancellableStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> { self.io(|stream| stream.read(buffer)) }
}
impl Write for CancellableStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> { self.io(|stream| stream.write(buffer)) }
    fn flush(&mut self) -> io::Result<()> { self.io(|stream| stream.flush()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_cannot_revive_an_old_operation() {
        let slot = Arc::new(OperationSlot::default());
        let first = slot.begin().unwrap();
        let old = first.cancel.clone();
        slot.cancel();
        assert!(slot.begin().is_err());
        drop(first);
        let second = slot.begin().unwrap();
        assert!(old.load(Ordering::Acquire));
        assert!(!second.cancel.load(Ordering::Acquire));
        drop(second);
        assert!(slot.is_idle());
    }

    #[test]
    fn cancellation_interrupts_blocked_socket_reads() {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let _client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut server = CancellableStream::new(server, Some(cancel.clone())).unwrap();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            cancel.store(true, Ordering::Release);
        });
        let start = std::time::Instant::now();
        assert!(server.read_exact(&mut [0]).is_err());
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        worker.join().unwrap();
    }

    #[test]
    fn partial_frames_survive_short_read_timeouts() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        let worker = std::thread::spawn(move || {
            client.write_all(b"first").unwrap();
            std::thread::sleep(Duration::from_millis(450));
            client.write_all(b"last").unwrap();
        });
        let mut server = CancellableStream::new(server, None).unwrap();
        let mut frame = [0; 9];
        server.read_exact(&mut frame).unwrap();
        assert_eq!(&frame, b"firstlast");
        worker.join().unwrap();
    }
}
