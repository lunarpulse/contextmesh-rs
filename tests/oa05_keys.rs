//! OA-05 D-05-01 key/token custody evidence.

use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use contextmesh::crypto::SigningIdentity;
use contextmesh::error::KeyFileError;
use contextmesh::http::TokenSource;
use contextmesh::store::Store;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa05-keys-{tag}-{}-{}.secret",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[cfg(unix)]
fn mode_of(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o777
}

/// 05-K01: identity is stable across generate/load/restart.
#[tokio::test]
async fn persistent_identity_survives_reload() {
    let path = temp_path("identity");
    let author = SigningIdentity::generate_key_file(&path).expect("generate");
    let reloaded = SigningIdentity::load_key_file(&path).expect("load");
    assert_eq!(reloaded.author(), author);
    let again = SigningIdentity::load_key_file(&path).expect("reload");
    assert_eq!(again.author(), author);
    assert_eq!(mode_of(&path), 0o600);
    assert_eq!(std::fs::metadata(&path).expect("len").len(), 32);
}

/// 05-K02: hostile filesystem matrix fails closed without overwriting.
#[tokio::test]
async fn hostile_filesystem_matrix() {
    let path = temp_path("matrix");
    assert!(matches!(
        SigningIdentity::generate_key_file(&path),
        Ok(_) | Err(KeyFileError::Unavailable)
    ));
    assert!(matches!(
        SigningIdentity::generate_key_file(&path),
        Err(KeyFileError::AlreadyExists)
    ));
    let original = std::fs::read(&path).expect("seed");

    let link = temp_path("link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&path, &link).expect("symlink");
    assert!(matches!(
        SigningIdentity::generate_key_file(&link),
        Err(KeyFileError::AlreadyExists)
    ));
    assert!(matches!(
        SigningIdentity::load_key_file(&link),
        Err(KeyFileError::Malformed | KeyFileError::Unavailable)
    ));

    let permissive = temp_path("permissive");
    std::fs::write(&permissive, [7_u8; 32]).expect("write");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&permissive, std::fs::Permissions::from_mode(0o644))
            .expect("perm");
    }
    assert!(matches!(
        SigningIdentity::load_key_file(&permissive),
        Err(KeyFileError::InsecurePermissions)
    ));
    SigningIdentity::repair_key_file_permissions(&permissive).expect("repair");
    assert!(SigningIdentity::load_key_file(&permissive).is_ok());

    let short = temp_path("short");
    std::fs::write(&short, [1_u8; 31]).expect("short");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&short, std::fs::Permissions::from_mode(0o600)).expect("perm");
    }
    assert!(matches!(
        SigningIdentity::load_key_file(&short),
        Err(KeyFileError::Malformed)
    ));

    let nested = temp_path("nested").join("missing").join("key");
    assert!(matches!(
        SigningIdentity::generate_key_file(&nested),
        Err(KeyFileError::Unavailable)
    ));

    assert_eq!(std::fs::read(&path).expect("unchanged"), original);
}

/// 05-K03: no secret bytes ever reach output surfaces.
#[tokio::test]
async fn secrets_never_reach_outputs() {
    let path = temp_path("secret-scan");
    let author = SigningIdentity::generate_key_file(&path).expect("generate");
    let seed = std::fs::read(&path).expect("seed");
    let identity = SigningIdentity::load_key_file(&path).expect("load");

    let token_path = temp_path("token");
    SigningIdentity::generate_token_file(&token_path).expect("token");
    let token_text = std::fs::read_to_string(&token_path).expect("token text");
    assert!(token_text.starts_with("token1_") && token_text.len() == 50);
    assert_eq!(mode_of(&token_path), 0o600);

    let permissive = temp_path("echo-perm");
    std::fs::write(&permissive, seed.clone()).expect("copy");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&permissive, std::fs::Permissions::from_mode(0o666))
            .expect("perm");
    }
    let failure = match SigningIdentity::load_key_file(&permissive) {
        Err(error) => format!("{error:?}"),
        Ok(_) => panic!("insecure key file must not load"),
    };
    assert!(!failure.contains(&hex_prefix(&seed)));
    let _ = identity;
    let _ = author;

    // The token file loads through the frozen OA-04 bearer contract.
    let store = Store::open(temp_path("token-store").with_extension("db"))
        .await
        .expect("store");
    let server = contextmesh::http::SyncServer::bind(
        store,
        contextmesh::http::SyncServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            acknowledge_non_loopback_plaintext: false,
            token: TokenSource::file(token_path),
            limits: contextmesh::http::TransportLimits::default(),
        },
    )
    .await
    .expect("server accepts generated token");
    let _ = server.local_addr();
}

fn hex_prefix(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[test]
fn generated_token_is_canonical_base64url() {
    let path = temp_path("canonical");
    SigningIdentity::generate_token_file(&path).expect("token");
    let text = std::fs::read_to_string(&path).expect("text");
    let encoded = text.strip_prefix("token1_").expect("prefix");
    assert_eq!(encoded.len(), 43);
    assert!(URL_SAFE_NO_PAD.decode(encoded).is_ok());
}
