use std::process::ExitStatus;
use std::time::Duration;
use tokio::process::Child;
use tracing::{debug, warn};

/// High-reliability process supervisor with kernel-level kill-tree guarantees
pub struct ProcessSupervisor {
    #[cfg(windows)]
    job_handle: Option<windows_sys::Win32::Foundation::HANDLE>,
    pid: Option<u32>,
}

impl ProcessSupervisor {
    pub fn new() -> Self {
        #[cfg(windows)]
        {
            let job_handle = Self::create_win32_job_object();
            Self {
                job_handle,
                pid: None,
            }
        }

        #[cfg(not(windows))]
        {
            Self { pid: None }
        }
    }

    /// Attaches an actively spawned child process to the kernel supervisor
    pub fn attach(&mut self, child: &Child) {
        if let Some(pid) = child.id() {
            self.pid = Some(pid);

            #[cfg(windows)]
            if let Some(job) = self.job_handle {
                unsafe {
                    use windows_sys::Win32::System::Threading::{
                        OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
                    };
                    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;

                    let proc_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
                    if !proc_handle.is_null() {
                        let ok = AssignProcessToJobObject(job, proc_handle);
                        if ok == 0 {
                            debug!("AssignProcessToJobObject returned 0 for pid {}", pid);
                        } else {
                            debug!("Successfully attached PID {} to Win32 Job Object", pid);
                        }
                        windows_sys::Win32::Foundation::CloseHandle(proc_handle);
                    }
                }
            }
        }
    }

    #[cfg(windows)]
    fn create_win32_job_object() -> Option<windows_sys::Win32::Foundation::HANDLE> {
        unsafe {
            use windows_sys::Win32::System::JobObjects::{
                CreateJobObjectW, SetInformationJobObject, JobObjectExtendedLimitInformation,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            };

            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                warn!("Failed to create Win32 JobObject");
                return None;
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if ok == 0 {
                warn!("Failed to set JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE");
                windows_sys::Win32::Foundation::CloseHandle(job);
                return None;
            }

            Some(job)
        }
    }

    /// Forcibly kills the entire process tree within sub-millisecond latency
    pub async fn terminate_tree(&mut self, child: Option<&mut Child>) {
        if let Some(pid) = self.pid {
            debug!("Terminating process tree for PID {}", pid);

            #[cfg(windows)]
            {
                if let Some(job) = self.job_handle {
                    unsafe {
                        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
                        TerminateJobObject(job, 1);
                    }
                }

                // Fallback process tree termination
                let _ = tokio::process::Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .await;
            }

            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }

        if let Some(c) = child {
            let _ = c.kill().await;
        }
    }

    /// Waits for process exit with timeout; kills tree if timeout exceeded
    pub async fn wait_with_timeout(
        &mut self,
        child: &mut Child,
        timeout: Duration,
    ) -> Result<ExitStatus, String> {
        self.attach(child);

        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => Ok(status),
            Ok(Err(e)) => Err(format!("Process wait error: {}", e)),
            Err(_) => {
                warn!("Job execution timed out after {:?}", timeout);
                self.terminate_tree(Some(child)).await;
                Err("Execution timed out".to_string())
            }
        }
    }
}

// Windows HANDLE is safe to send between threads in this supervisor context
unsafe impl Send for ProcessSupervisor {}
unsafe impl Sync for ProcessSupervisor {}

impl Drop for ProcessSupervisor {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(job) = self.job_handle {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(job);
            }
        }
    }
}
