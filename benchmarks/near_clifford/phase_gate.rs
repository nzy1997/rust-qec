//! Diagnostic-only perf window. The warm loop includes its RNG and output drops.
#[cfg(unix)]
mod unix {
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::FileTypeExt;
    use std::path::Path;
    use std::time::{Duration, Instant};

    pub struct Gate {
        control: File,
        acknowledgement: File,
        report: File,
        start_ns: u64,
        enable_ack: [u8; 5],
    }

    fn monotonic_ns() -> Result<u64, String> {
        let mut stamp = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // The OS writes this initialized timespec; inspect it only on success.
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut stamp) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let seconds = u64::try_from(stamp.tv_sec).map_err(|_| "negative phase clock")?;
        let nanos = u64::try_from(stamp.tv_nsec).map_err(|_| "negative phase clock")?;
        if nanos >= 1_000_000_000 {
            return Err("invalid phase clock nanoseconds".into());
        }
        seconds
            .checked_mul(1_000_000_000)
            .and_then(|s| s.checked_add(nanos))
            .ok_or_else(|| "phase clock overflow".into())
    }

    fn exchange(
        control: &mut File,
        acknowledgement: &mut File,
        command: &[u8],
        timeout: Duration,
    ) -> Result<[u8; 5], String> {
        control
            .write_all(command)
            .and_then(|_| control.flush())
            .map_err(|e| e.to_string())?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or("phase acknowledgement deadline overflow")?;
        // Linux perf writes sizeof("ack\n"), including its terminating NUL.
        // Consume the entire frame before a subsequent command can be sent.
        let mut reply = [0u8; 5];
        for byte in &mut reply {
            loop {
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .ok_or("perf phase acknowledgement timed out")?;
                let millis = i32::try_from(remaining.as_millis().max(1))
                    .map_err(|_| "phase timeout overflow")?;
                let mut request = libc::pollfd {
                    fd: acknowledgement.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // poll borrows one initialized descriptor for the duration of this call.
                let count = unsafe { libc::poll(&mut request, 1, millis) };
                if count < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error.to_string());
                }
                if count == 0 {
                    return Err("perf phase acknowledgement timed out".into());
                }
                if request.revents & libc::POLLIN == 0 {
                    return Err("perf phase acknowledgement channel closed".into());
                }
                acknowledgement
                    .read_exact(std::slice::from_mut(byte))
                    .map_err(|e| e.to_string())?;
                break;
            }
        }
        if reply != *b"ack\n\0" {
            return Err("invalid perf phase acknowledgement".into());
        }
        Ok(reply)
    }

    impl Gate {
        pub fn from_env() -> Result<Option<Self>, String> {
            let paths = [
                "RSTIM_PHASE_CONTROL",
                "RSTIM_PHASE_ACK",
                "RSTIM_PHASE_REPORT",
            ]
            .map(std::env::var_os);
            if paths.iter().all(Option::is_none) {
                return Ok(None);
            }
            let [Some(control), Some(acknowledgement), Some(report)] = paths else {
                return Err("phase capture needs CONTROL, ACK and REPORT paths together".into());
            };
            Self::open(
                Path::new(&control),
                Path::new(&acknowledgement),
                Path::new(&report),
            )
            .map(Some)
        }

        fn open(control: &Path, acknowledgement: &Path, report: &Path) -> Result<Self, String> {
            if control == acknowledgement || control == report || acknowledgement == report {
                return Err("phase capture paths must be distinct".into());
            }
            for path in [control, acknowledgement] {
                if !path.is_absolute()
                    || !path
                        .symlink_metadata()
                        .map_err(|e| e.to_string())?
                        .file_type()
                        .is_fifo()
                {
                    return Err(
                        "phase control and acknowledgement must be absolute FIFO paths".into(),
                    );
                }
            }
            if !report.is_absolute() {
                return Err("phase report must be an absolute path".into());
            }
            let mut report = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(report)
                .map_err(|e| e.to_string())?;
            report
                .write_all(b"{\"state\":\"waiting for enable\",\"completed\":false}\n")
                .map_err(|e| e.to_string())?;
            let mut control = OpenOptions::new()
                .read(true)
                .write(true)
                .open(control)
                .map_err(|e| e.to_string())?;
            let mut acknowledgement = OpenOptions::new()
                .read(true)
                .write(true)
                .open(acknowledgement)
                .map_err(|e| e.to_string())?;
            let enable_ack = exchange(
                &mut control,
                &mut acknowledgement,
                b"enable\n",
                Duration::from_secs(15),
            )?;
            let start_ns = monotonic_ns()?;
            Ok(Self {
                control,
                acknowledgement,
                report,
                start_ns,
                enable_ack,
            })
        }

        pub fn finish(mut self) -> Result<(), String> {
            let end_ns = monotonic_ns()?;
            if end_ns <= self.start_ns {
                return Err("nonpositive warm phase span".into());
            }
            let disable_ack = exchange(
                &mut self.control,
                &mut self.acknowledgement,
                b"disable\n",
                Duration::from_secs(15),
            )?;
            let value = serde_json::json!({"schema":"rstim.perf-warm-loop.v1", "completed":true,
                "pid":std::process::id(), "clock":"CLOCK_MONOTONIC", "start_ns":self.start_ns,
                "end_ns":end_ns, "acknowledgements":{"enable":self.enable_ack,"disable":disable_ack},
                "scope":"warm observation loop including RNG, output drops, timer and observation bookkeeping; excludes cold setup and final serialization"});
            self.report
                .seek(SeekFrom::Start(0))
                .and_then(|_| self.report.set_len(0))
                .and_then(|_| self.report.write_all(value.to_string().as_bytes()))
                .and_then(|_| self.report.write_all(b"\n"))
                .and_then(|_| self.report.flush())
                .map_err(|e| e.to_string())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::fd::OwnedFd;
        use std::os::unix::net::UnixStream;
        use std::thread;

        fn channel() -> (File, File, UnixStream) {
            let (client, peer) = UnixStream::pair().unwrap();
            let control = File::from(OwnedFd::from(client.try_clone().unwrap()));
            (control, File::from(OwnedFd::from(client)), peer)
        }

        #[test]
        fn phase_command_waits_for_fragmented_acknowledgement() {
            let (mut control, mut acknowledgement, mut peer) = channel();
            let actor = thread::spawn(move || {
                let mut command = [0; 7];
                peer.read_exact(&mut command).unwrap();
                assert_eq!(&command, b"enable\n");
                peer.write_all(b"ac").unwrap();
                thread::sleep(Duration::from_millis(20));
                peer.write_all(b"k\n").unwrap();
                thread::sleep(Duration::from_millis(20));
                peer.write_all(b"\0").unwrap();
            });
            let reply = exchange(
                &mut control,
                &mut acknowledgement,
                b"enable\n",
                Duration::from_secs(1),
            )
            .unwrap();
            assert_eq!(&reply, b"ack\n\0");
            actor.join().unwrap();
        }

        #[test]
        fn phase_command_rejects_wrong_acknowledgement() {
            for invalid in [*b"bad\n\0", *b"ack\nx"] {
                let (mut control, mut acknowledgement, mut peer) = channel();
                let actor = thread::spawn(move || {
                    let mut command = [0; 8];
                    peer.read_exact(&mut command).unwrap();
                    assert_eq!(&command, b"disable\n");
                    peer.write_all(&invalid).unwrap();
                });
                assert!(
                    exchange(
                        &mut control,
                        &mut acknowledgement,
                        b"disable\n",
                        Duration::from_secs(1)
                    )
                    .unwrap_err()
                    .contains("invalid")
                );
                actor.join().unwrap();
            }
        }

        #[test]
        fn partial_acknowledgement_has_a_total_deadline() {
            for partial in [&b"ac"[..], &b"ack\n"[..]] {
                let (mut control, mut acknowledgement, mut peer) = channel();
                let actor = thread::spawn(move || {
                    let mut command = [0; 7];
                    peer.read_exact(&mut command).unwrap();
                    assert_eq!(&command, b"enable\n");
                    peer.write_all(partial).unwrap();
                    thread::sleep(Duration::from_millis(100));
                });
                assert!(
                    exchange(
                        &mut control,
                        &mut acknowledgement,
                        b"enable\n",
                        Duration::from_millis(30)
                    )
                    .unwrap_err()
                    .contains("timed out")
                );
                actor.join().unwrap();
            }
        }

        #[test]
        fn fifo_gate_retains_a_completed_monotonic_warm_span() {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;

            struct Fixture(std::path::PathBuf);
            impl Drop for Fixture {
                fn drop(&mut self) {
                    for name in ["control", "ack", "phase.json"] {
                        let _ = std::fs::remove_file(self.0.join(name));
                    }
                    let _ = std::fs::remove_dir(&self.0);
                }
            }
            let fixture = Fixture(std::env::temp_dir().join(format!(
                "rstim-perf-gate-{}-{}",
                std::process::id(),
                monotonic_ns().unwrap()
            )));
            std::fs::create_dir(&fixture.0).unwrap();
            let control_path = fixture.0.join("control");
            let ack_path = fixture.0.join("ack");
            let report_path = fixture.0.join("phase.json");
            for path in [&control_path, &ack_path] {
                let name = CString::new(path.as_os_str().as_bytes()).unwrap();
                // This test owns its fresh directory and creates two named FIFOs.
                assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            }
            let mut control = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&control_path)
                .unwrap();
            let mut ack = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&ack_path)
                .unwrap();
            let actor = thread::spawn(move || {
                for expected in [&b"enable\n"[..], &b"disable\n"[..]] {
                    let mut actual = Vec::new();
                    for _ in expected {
                        let mut request = libc::pollfd {
                            fd: control.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        };
                        // A bounded wait keeps a failed test from hanging its peer.
                        assert_eq!(unsafe { libc::poll(&mut request, 1, 2000) }, 1);
                        let mut byte = [0];
                        control.read_exact(&mut byte).unwrap();
                        actual.push(byte[0]);
                    }
                    assert_eq!(actual, expected);
                    ack.write_all(b"ac").unwrap();
                    thread::sleep(Duration::from_millis(2));
                    ack.write_all(b"k\n").unwrap();
                    thread::sleep(Duration::from_millis(2));
                    ack.write_all(b"\0").unwrap();
                }
            });
            let before = monotonic_ns().unwrap();
            let gate = Gate::open(&control_path, &ack_path, &report_path).unwrap();
            thread::sleep(Duration::from_millis(5));
            gate.finish().unwrap();
            let after = monotonic_ns().unwrap();
            actor.join().unwrap();
            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();
            assert_eq!(report["completed"], true);
            assert_eq!(report["pid"], std::process::id());
            assert_eq!(report["clock"], "CLOCK_MONOTONIC");
            assert_eq!(
                report["acknowledgements"]["enable"],
                serde_json::json!([97, 99, 107, 10, 0])
            );
            assert_eq!(
                report["acknowledgements"]["disable"],
                serde_json::json!([97, 99, 107, 10, 0])
            );
            assert!(before <= report["start_ns"].as_u64().unwrap());
            assert!(report["start_ns"].as_u64().unwrap() < report["end_ns"].as_u64().unwrap());
            assert!(report["end_ns"].as_u64().unwrap() <= after);
        }
    }
}

#[cfg(unix)]
pub use unix::Gate;

#[cfg(not(unix))]
pub struct Gate;
#[cfg(not(unix))]
impl Gate {
    pub fn from_env() -> Result<Option<Self>, String> {
        if [
            "RSTIM_PHASE_CONTROL",
            "RSTIM_PHASE_ACK",
            "RSTIM_PHASE_REPORT",
        ]
        .iter()
        .any(|name| std::env::var_os(name).is_some())
        {
            return Err("perf phase capture requires Unix FIFOs".into());
        }
        Ok(None)
    }
    pub fn finish(self) -> Result<(), String> {
        Err("perf phase capture requires Unix FIFOs".into())
    }
}
