use async_trait::async_trait;
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE},
        Security::{
            AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW,
            SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
        },
        System::{
            Shutdown::{
                InitiateShutdownW, SHUTDOWN_FORCE_SELF, SHUTDOWN_POWEROFF, SHUTDOWN_REASON,
            },
            Threading::{GetCurrentProcess, OpenProcessToken},
        },
    },
    core::w,
};

use super::{PowerController, PowerError};

pub struct WindowsPowerController;

#[async_trait]
impl PowerController for WindowsPowerController {
    async fn shutdown(&self) -> Result<(), PowerError> {
        enable_shutdown_privilege()?;
        let result = unsafe {
            InitiateShutdownW(
                None,
                None,
                0,
                SHUTDOWN_POWEROFF | SHUTDOWN_FORCE_SELF,
                SHUTDOWN_REASON(0x0004_0000),
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(PowerError {
                message: format!("InitiateShutdownW failed with Windows error {result}"),
            })
        }
    }
}

fn enable_shutdown_privilege() -> Result<(), PowerError> {
    let mut token = HANDLE::default();
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
    }
    .map_err(|error| PowerError {
        message: format!("could not open service token: {error}"),
    })?;
    let operation = (|| {
        let mut luid = Default::default();
        unsafe { LookupPrivilegeValueW(None, w!("SeShutdownPrivilege"), &mut luid) }.map_err(
            |error| PowerError {
                message: format!("could not find SeShutdownPrivilege: {error}"),
            },
        )?;
        let privileges = TOKEN_PRIVILEGES {
            PrivilegeCount: 1,
            Privileges: [LUID_AND_ATTRIBUTES {
                Luid: luid,
                Attributes: SE_PRIVILEGE_ENABLED,
            }],
        };
        unsafe { AdjustTokenPrivileges(token, false, Some(&privileges), 0, None, None) }.map_err(
            |error| PowerError {
                message: format!("could not enable SeShutdownPrivilege: {error}"),
            },
        )?;
        if unsafe { GetLastError() } == ERROR_NOT_ALL_ASSIGNED {
            return Err(PowerError {
                message: "the service account does not hold SeShutdownPrivilege".into(),
            });
        }
        Ok(())
    })();
    unsafe {
        let _ = CloseHandle(token);
    }
    operation
}
