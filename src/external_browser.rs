//! Open a web address through the OS browser association, never a file manager
//! or a shell command. Callers retain authorization and live-attempt ownership.

pub(crate) fn open(address: &str) -> Result<(), String> {
    open_with(address, platform::open)
}

fn open_with(address: &str, launch: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
    let invalid = "This address cannot be opened in a web browser.";
    if address != address.trim() || address.chars().any(char::is_control) {
        return Err(invalid.into());
    }
    let parsed = url::Url::parse(address).map_err(|_| invalid)?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(invalid.into());
    }
    // Preserve the complete authorization query; do not convert it to a path,
    // command line, or a newly serialized URL. Never include it in an error.
    launch(address)
}

#[cfg(windows)]
mod platform {
    use windows::{
        Win32::UI::{
            Shell::{SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
        core::{HSTRING, PCWSTR, w},
    };

    pub(super) fn open(address: &str) -> Result<(), String> {
        dispatch(address, |info| unsafe { ShellExecuteExW(info) })
    }

    fn dispatch(
        address: &str,
        execute: impl FnOnce(&mut SHELLEXECUTEINFOW) -> windows::core::Result<()>,
    ) -> Result<(), String> {
        let address = HSTRING::from(address);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: w!("open"),
            lpFile: PCWSTR(address.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        execute(&mut info).map_err(|error| {
            format!(
                "Could not open the default browser (Windows error 0x{:08X}). Check your default browser in Windows Settings and try again.",
                error.code().0 as u32
            )
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn browser_dispatch_preserves_oauth_url_without_command_parameters() {
            let address = "https://auth.openai.com/authorize?state=a%2Bb%26c&scope=openid%20profile&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback";
            dispatch(address, |info| {
                assert_eq!(unsafe { info.lpFile.to_string().unwrap() }, address);
                assert_eq!(unsafe { info.lpVerb.to_string().unwrap() }, "open");
                assert!(info.lpParameters.is_null());
                assert!(info.lpDirectory.is_null());
                assert_eq!(info.nShow, SW_SHOWNORMAL.0);
                assert_eq!(info.fMask, SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI);
                Ok(())
            })
            .unwrap();
        }

        #[test]
        fn browser_dispatch_failure_is_reported_without_the_authorization_url() {
            let error = dispatch("https://example.com/?state=PRIVATE_STATE", |_| {
                Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                    0x80070483_u32 as i32,
                )))
            })
            .unwrap_err();
            assert!(error.contains("default browser") && error.contains("80070483"));
            assert!(!error.contains("PRIVATE_STATE") && !error.contains("example.com"));
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub(super) fn open(address: &str) -> Result<(), String> {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(program)
            .arg(address)
            .spawn()
            .map(|_| ())
            .map_err(|_| {
                "Could not open the default browser. Check your browser settings and try again."
                    .into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_opener_validates_web_addresses_before_launching() {
        for address in [
            "https://auth.openai.com/authorize?state=one%2Btwo&scope=openid%20profile",
            "https://example.com/device?code=a,b&value=%25#complete",
            "http://127.0.0.1:1455/authorize?state=fixture",
        ] {
            let mut launched = false;
            open_with(address, |actual| {
                assert_eq!(actual, address);
                launched = true;
                Ok(())
            })
            .unwrap();
            assert!(launched);
        }
        for address in [
            "file:///C:/Users",
            "C:\\Users",
            "javascript:alert(1)",
            "data:text/html,test",
            "https://user:password@example.com",
            "https://",
            "//example.com",
            "https://example.com/\0PRIVATE",
            "https://example.com/\nPRIVATE",
        ] {
            let error = open_with(address, |_| panic!("Invalid address was launched")).unwrap_err();
            assert!(!error.contains(address) && !error.contains("PRIVATE"));
        }
    }
}
