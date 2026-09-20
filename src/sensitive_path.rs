use std::path::{Component, Path};

/// Returns whether a path is known to commonly contain credentials or secrets.
///
/// This intentionally uses a conservative, path-only policy. Callers must not
/// infer that a `false` result proves a file is safe; it only means the path did
/// not match one of Supervisor's known secret locations or file names.
pub(crate) fn is_sensitive_path(path: &Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_ascii_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if components.iter().any(|component| {
        matches!(
            component.as_str(),
            ".ssh"
                | ".gnupg"
                | ".aws"
                | ".azure"
                | ".kube"
                | ".docker"
                | ".oci"
                | ".terraform.d"
                | ".password-store"
                | ".secrets"
                | "secrets"
        )
    }) || contains_component_sequence(&components, &[".config", "gcloud"])
        || contains_component_sequence(&components, &[".config", "gh"])
        || contains_component_sequence(&components, &[".config", "glab"])
        || contains_component_sequence(&components, &[".config", "rclone"])
        || contains_component_sequence(&components, &[".config", "op"])
        || contains_component_sequence(&components, &[".config", "1password"])
        || contains_component_sequence(&components, &[".local", "share", "keyrings"])
    {
        return true;
    }

    let Some(name) = components.last() else {
        return false;
    };

    // `.env*` includes conventional variants such as `.env.local` and `.envrc`.
    name.starts_with(".env")
        || matches!(
            name.as_str(),
            ".dev.vars"
                | ".npmrc"
                | ".pypirc"
                | ".netrc"
                | "_netrc"
                | ".git-credentials"
                | ".boto"
                | ".s3cfg"
                | ".terraformrc"
                | "terraform.rc"
                | "credentials.tfrc.json"
                | "credentials"
                | "credentials.json"
                | "credentials.yaml"
                | "credentials.yml"
                | "application_default_credentials.json"
                | "service-account.json"
                | "service_account.json"
                | "auth.json"
                | "rclone.conf"
                | "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
                | "known_hosts"
                | "login data"
                | "logins.json"
                | "key4.db"
                | "cookies.sqlite"
        )
        || is_numbered_or_named_private_key(name)
        || is_structured_secret_file(name)
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".pfx")
        || name.ends_with(".p12")
        || name.ends_with(".ppk")
        || name.ends_with(".jks")
        || name.ends_with(".keystore")
        || name.ends_with(".keytab")
        || name.ends_with(".kdbx")
}

fn contains_component_sequence(components: &[String], expected: &[&str]) -> bool {
    components.windows(expected.len()).any(|window| {
        window
            .iter()
            .zip(expected)
            .all(|(component, expected)| component == expected)
    })
}

fn is_numbered_or_named_private_key(name: &str) -> bool {
    !name.ends_with(".pub")
        && ["id_rsa_", "id_dsa_", "id_ecdsa_", "id_ed25519_"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

fn is_structured_secret_file(name: &str) -> bool {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(extension, "json" | "yaml" | "yml" | "toml")
        && (matches!(stem, "secret" | "secrets" | "credential" | "credentials")
            || stem.ends_with("service-account")
            || stem.ends_with("service_account"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_environment_and_common_credential_files() {
        for path in [
            ".env",
            ".env.local",
            ".envrc",
            "nested/.dev.vars",
            "credentials.json",
            "production-service-account.json",
            "private.pem",
            "deploy.ppk",
            "passwords.kdbx",
        ] {
            assert!(
                is_sensitive_path(Path::new(path)),
                "expected {path} to be sensitive"
            );
        }
    }

    #[test]
    fn detects_secret_directories_and_cloud_configuration() {
        for path in [
            ".ssh/id_ed25519",
            "home/.aws/config",
            "home/.docker/config.json",
            "home/.config/gcloud/configurations/config_default",
            "home/.config/gh/hosts.yml",
            "home/.local/share/keyrings/login.keyring",
            "project/secrets/application.toml",
        ] {
            assert!(
                is_sensitive_path(Path::new(path)),
                "expected {path} to be sensitive"
            );
        }
    }

    #[test]
    fn permits_public_keys_and_ordinary_project_files() {
        for path in [
            "src/main.rs",
            "config/default.toml",
            "docs/environment.md",
            "keys/id_ed25519.pub",
            "public/certificate.crt",
            "src/secret.rs",
        ] {
            assert!(
                !is_sensitive_path(Path::new(path)),
                "expected {path} to be allowed"
            );
        }
    }
}
