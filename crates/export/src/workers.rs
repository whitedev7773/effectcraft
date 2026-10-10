//! Whole-frame jobs stay on OS threads. Rayon is reserved for work *inside* each frame:
//! scheduling whole frames on it lets an inner join steal another frame and recurse.

use crate::{ExportError, Result};

fn worker_error(payload: Box<dyn std::any::Any + Send>) -> ExportError {
    let message = payload.downcast_ref::<String>().map(String::as_str).or_else(|| payload.downcast_ref::<&str>().copied()).unwrap_or("unknown panic");
    ExportError::Render(format!("frame worker failed: {message}"))
}

/// Bounded, ordered map, with all workers joined even after a spawn/render failure.
pub(crate) fn ordered_map<T: Send, F: Fn(u64) -> T + Sync>(ks: &[u64], workers: usize, f: F) -> Result<Vec<T>> {
    if ks.is_empty() {
        return Ok(Vec::new());
    }
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        let mut failure = None;
        let chunk = ks.len().div_ceil(workers.clamp(1, 4));
        for ks in ks.chunks(chunk) {
            let f = &f;
            match std::thread::Builder::new()
                .name("export-frame".into())
                .stack_size(8 << 20)
                .spawn_scoped(scope, move || ks.iter().map(|&k| f(k)).collect::<Vec<T>>())
            {
                Ok(handle) => handles.push(handle),
                Err(e) => {
                    failure = Some(ExportError::Render(format!("cannot start frame worker: {e}")));
                    break;
                }
            }
        }
        let mut out = Vec::with_capacity(ks.len());
        for handle in handles {
            match handle.join() {
                Ok(values) => out.extend(values),
                Err(e) => failure = Some(worker_error(e)),
            }
        }
        match failure {
            Some(e) => Err(e),
            None => Ok(out),
        }
    })
}

/// Render at most one queued batch ahead while the caller encodes the preceding batch.
/// Encoder/muxer/progress callbacks stay on the caller; their failure drops the receiver
/// before joining, so a producer blocked on a full channel always wakes on cancellation.
pub(crate) fn batches<T: Send>(
    start: u64,
    total: u64,
    batch: u64,
    render: impl Fn(u64, u64) -> Result<Vec<T>> + Sync + Send,
    mut consume: impl FnMut(u64, u64, Vec<T>) -> Result<()>,
) -> Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let stopped = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let stop = &stopped;
        let worker = std::thread::Builder::new()
            .name("export-prefetch".into())
            .spawn_scoped(scope, move || {
                let mut i = start;
                while i < total && !stop.load(Ordering::Relaxed) {
                    let end = i.saturating_add(batch.max(1)).min(total);
                    let frames = render(i, end);
                    let failed = frames.is_err();
                    if tx.send((i, end, frames)).is_err() || failed {
                        break;
                    }
                    i = end;
                }
            })
            .map_err(|e| ExportError::Render(format!("cannot start export prefetch: {e}")))?;
        let result = (|| {
            let mut next = start;
            while next < total {
                let (i, end, frames) = rx.recv().map_err(|e| ExportError::Render(format!("frame producer disconnected: {e}")))?;
                if i != next || end <= i || end > total {
                    return Err(ExportError::Render("invalid frame batch order".into()));
                }
                consume(i, end, frames?)?;
                next = end;
            }
            Ok(())
        })();
        stopped.store(true, Ordering::Relaxed);
        drop(rx);
        let joined = worker.join().map_err(worker_error);
        result.and(joined)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn frames_yield_without_recursively_stealing_other_frames() {
        let depth = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let ks: Vec<_> = (0..1024).collect();
        let out = ordered_map(&ks, 1, |k| {
            assert!(rayon::current_thread_index().is_none());
            let d = depth.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(d, Ordering::SeqCst);
            rayon::join(
                || {
                    rayon::yield_now();
                },
                || {
                    rayon::yield_now();
                },
            );
            depth.fetch_sub(1, Ordering::SeqCst);
            k
        })
        .unwrap();
        assert_eq!(out, ks);
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn failed_worker_is_an_error_and_other_workers_are_joined() {
        let done = AtomicUsize::new(0);
        let result = ordered_map(&[0, 1], 2, |k| {
            if k == 0 {
                panic!("bad synthetic frame");
            }
            done.fetch_add(1, Ordering::SeqCst);
            k
        });
        assert!(matches!(result, Err(ExportError::Render(_))));
        assert_eq!(done.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn overlap_preserves_order_and_stops_on_consumer_failure() {
        let mut seen = Vec::new();
        batches(
            3,
            25,
            4,
            |i, end| Ok((i..end).collect::<Vec<_>>()),
            |_, _, frames| {
                seen.extend(frames);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(seen, (3..25).collect::<Vec<_>>());
        let result = batches(0, 100, 2, |i, end| Ok((i..end).collect::<Vec<_>>()), |_, _, _| Err(ExportError::Cancelled));
        assert!(matches!(result, Err(ExportError::Cancelled)));
    }

    #[test]
    fn producer_failure_is_reported() {
        let result = batches::<u64>(0, 5, 2, |_, _| Err(ExportError::Render("synthetic failure".into())), |_, _, _| Ok(()));
        assert!(matches!(result, Err(ExportError::Render(_))));
    }
}
